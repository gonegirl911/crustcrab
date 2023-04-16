pub mod action;
pub mod block;
pub mod chunk;
pub mod light;

use self::{
    action::{ActionStore, BlockAction},
    block::{Block, BlockArea},
    chunk::{generator::ChunkGenerator, light::ChunkAreaLight, Chunk, ChunkArea},
    light::ChunkMapLight,
};
use crate::{
    client::{game::world::BlockVertex, ClientEvent},
    server::{
        event_loop::{Event, EventHandler},
        game::player::{
            ray::{BlockIntersection, Ray},
            Player, WorldArea,
        },
        ServerEvent, ServerSettings,
    },
    shared::utils,
};
use flume::Sender;
use nalgebra::{Point2, Point3};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    collections::{hash_map::Entry, LinkedList},
    ops::{Deref, Index, Range},
    sync::Arc,
};

#[derive(Default)]
pub struct World {
    chunks: ChunkStore,
    generator: ChunkGenerator,
    actions: ActionStore,
    light: ChunkMapLight,
    hovered_block: Option<BlockIntersection>,
    reach: Range<f32>,
}

impl World {
    pub fn new(settings: &ServerSettings) -> Self {
        Self {
            reach: settings.reach.clone(),
            ..Default::default()
        }
    }

    fn load_many<I>(&mut self, points: I) -> Vec<Point3<i32>>
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        points
            .into_iter()
            .map(|coords| {
                if self.chunks.load(coords) {
                    Ok(coords)
                } else {
                    Err(coords)
                }
            })
            .collect::<Vec<_>>()
            .into_par_iter()
            .map(|coords| coords.map_err(|coords| self.gen(coords).map(|cell| (coords, cell))))
            .collect::<LinkedList<_>>()
            .into_iter()
            .filter_map(|entry| match entry {
                Ok(coords) => Some(coords),
                Err(Some((coords, cell))) => {
                    self.chunks.insert(coords, cell);
                    Some(coords)
                }
                Err(None) => None,
            })
            .collect()
    }

    fn unload_many<I>(&mut self, points: I) -> Vec<Point3<i32>>
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        points
            .into_iter()
            .filter(|coords| self.chunks.unload(*coords))
            .collect()
    }

    fn apply(
        &mut self,
        coords: Point3<i64>,
        action: BlockAction,
        server_tx: Sender<ServerEvent>,
        ray: Ray,
    ) {
        let Ok((load, unload)) = self.chunks.apply(coords, &action) else { return };
        let points = [load, unload].into_iter().flatten().collect();
        let light_updates = self.light.apply(&self.chunks, coords, &action);
        let updates = Self::updates(&points, light_updates.into_iter().chain([coords]), false);

        self.actions.insert(coords, action);

        self.handle(&WorldEvent::BlockHoverRequested { ray }, server_tx.clone());

        self.send_unloads(unload, server_tx.clone());
        self.send_loads(load, server_tx.clone(), true);
        self.send_updates(updates, server_tx, true);
    }

    fn send_loads<I>(&self, points: I, server_tx: Sender<ServerEvent>, is_important: bool)
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        self.send_events(
            points
                .into_iter()
                .map(|coords| self.load_event(coords, is_important)),
            server_tx,
        );
    }

    fn par_send_loads<I>(&self, points: I, server_tx: Sender<ServerEvent>, is_important: bool)
    where
        I: IntoParallelIterator<Item = Point3<i32>>,
    {
        self.send_events(
            points
                .into_par_iter()
                .map(|coords| self.load_event(coords, is_important))
                .collect::<LinkedList<_>>(),
            server_tx,
        );
    }

    fn send_unloads<I>(&self, points: I, server_tx: Sender<ServerEvent>)
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        self.send_events(
            points.into_iter().map(|coords| self.unload_event(coords)),
            server_tx,
        );
    }

    fn send_updates<I: IntoIterator<Item = Point3<i32>>>(
        &self,
        points: I,
        server_tx: Sender<ServerEvent>,
        is_important: bool,
    ) {
        self.send_events(
            points
                .into_iter()
                .filter_map(|coords| self.update_event(coords, is_important)),
            server_tx,
        );
    }

    fn par_send_updates<I: IntoParallelIterator<Item = Point3<i32>>>(
        &self,
        points: I,
        server_tx: Sender<ServerEvent>,
        is_important: bool,
    ) {
        self.send_events(
            points
                .into_par_iter()
                .filter_map(|coords| self.update_event(coords, is_important))
                .collect::<LinkedList<_>>(),
            server_tx,
        );
    }

    fn gen(&self, coords: Point3<i32>) -> Option<ChunkCell> {
        let mut chunk = self.generator.gen(coords);
        for (coords, action) in self.actions.actions(coords) {
            chunk.apply(coords, action);
        }
        ChunkCell::new(chunk)
    }

    fn load_event(&self, coords: Point3<i32>, is_important: bool) -> ServerEvent {
        ServerEvent::ChunkLoaded {
            coords,
            data: Arc::new(ChunkData {
                chunk: self.chunks[coords].clone(),
                area: self.chunks.chunk_area(coords),
                area_light: self.light.chunk_area_light(coords),
            }),
            is_important,
        }
    }

    fn unload_event(&self, coords: Point3<i32>) -> ServerEvent {
        ServerEvent::ChunkUnloaded { coords }
    }

    fn update_event(&self, coords: Point3<i32>, is_important: bool) -> Option<ServerEvent> {
        Some(ServerEvent::ChunkUpdated {
            coords,
            data: Arc::new(ChunkData {
                chunk: self.chunks.get(coords)?.clone(),
                area: self.chunks.chunk_area(coords),
                area_light: self.light.chunk_area_light(coords),
            }),
            is_important,
        })
    }

    fn send_events<I>(&self, events: I, server_tx: Sender<ServerEvent>)
    where
        I: IntoIterator<Item = ServerEvent>,
    {
        for event in events {
            server_tx.send(event).unwrap_or_else(|_| unreachable!());
        }
    }

    fn updates<B: IntoIterator<Item = Point3<i64>>>(
        points: &FxHashSet<Point3<i32>>,
        block_updates: B,
        include_outline: bool,
    ) -> FxHashSet<Point3<i32>> {
        Self::block_area_points(block_updates)
            .map(Self::chunk_coords)
            .chain(
                include_outline
                    .then_some(Self::chunk_area_points(points))
                    .into_iter()
                    .flatten(),
            )
            .filter(|coords| !points.contains(coords))
            .collect()
    }

    fn chunk_area_points(
        points: &FxHashSet<Point3<i32>>,
    ) -> impl Iterator<Item = Point3<i32>> + '_ {
        points
            .iter()
            .flat_map(|coords| ChunkArea::chunk_deltas().map(move |delta| coords + delta.cast()))
    }

    fn block_area_points<I>(block_updates: I) -> impl Iterator<Item = Point3<i64>>
    where
        I: IntoIterator<Item = Point3<i64>>,
    {
        block_updates
            .into_iter()
            .flat_map(|coords| BlockArea::deltas().map(move |delta| coords + delta.cast()))
    }

    pub fn chunk_coords(coords: Point3<i64>) -> Point3<i32> {
        coords.map(|c| utils::div_floor(c, Chunk::DIM as i64) as i32)
    }

    pub fn block_coords(coords: Point3<i64>) -> Point3<u8> {
        coords.map(|c| c.rem_euclid(Chunk::DIM as i64) as u8)
    }

    pub fn coords(chunk_coords: Point3<i32>, block_coords: Point3<u8>) -> Point3<i64> {
        chunk_coords.cast() * Chunk::DIM as i64 + block_coords.coords.cast()
    }
}

impl EventHandler<WorldEvent> for World {
    type Context<'a> = Sender<ServerEvent>;

    fn handle(&mut self, event: &WorldEvent, server_tx: Self::Context<'_>) {
        match event {
            WorldEvent::InitialRenderRequested { area, ray } => {
                let mut loads = self.load_many(area.points());

                loads.par_sort_unstable_by_key(|coords| {
                    utils::magnitude_squared(coords - area.center)
                });

                self.handle(
                    &WorldEvent::BlockHoverRequested { ray: *ray },
                    server_tx.clone(),
                );

                self.par_send_loads(loads, server_tx, false);
            }
            WorldEvent::WorldAreaChanged { prev, curr, ray } => {
                let unloads = self.unload_many(prev.exclusive_points(curr));
                let loads = self.load_many(curr.exclusive_points(prev));
                let points = loads.iter().chain(&unloads).copied().collect();
                let updates = Self::updates(&points, [], true);

                self.handle(
                    &WorldEvent::BlockHoverRequested { ray: *ray },
                    server_tx.clone(),
                );

                self.send_unloads(unloads, server_tx.clone());
                self.par_send_loads(loads, server_tx.clone(), false);
                self.par_send_updates(updates, server_tx, false);
            }
            WorldEvent::BlockHoverRequested { ray } => {
                let hovered_block =
                    ray.cast(self.reach.clone())
                        .find(|BlockIntersection { coords, .. }| {
                            self.chunks.block(*coords) != Block::Air
                        });

                if self.hovered_block != hovered_block {
                    self.hovered_block = hovered_block;
                    server_tx
                        .send(ServerEvent::BlockHovered {
                            coords: self.hovered_block.map(|data| data.coords),
                        })
                        .unwrap_or_else(|_| unreachable!());
                }
            }
            WorldEvent::BlockPlaced { block, ray } => {
                if let Some(BlockIntersection { coords, normal }) = self.hovered_block {
                    self.apply(coords + normal, BlockAction::Place(*block), server_tx, *ray);
                }
            }
            WorldEvent::BlockDestroyed { ray } => {
                if let Some(BlockIntersection { coords, .. }) = self.hovered_block {
                    self.apply(coords, BlockAction::Destroy, server_tx, *ray);
                }
            }
        }
    }
}

#[derive(Default)]
pub struct ChunkStore {
    cells: FxHashMap<Point3<i32>, ChunkCell>,
    y_ranges: FxHashMap<Point2<i32>, Range<i32>>,
}

impl ChunkStore {
    fn chunk_area(&self, coords: Point3<i32>) -> ChunkArea {
        let coords = World::coords(coords, Default::default());
        ChunkArea::from_fn(|delta| self.block(coords + delta.cast()).data().is_opaque())
    }

    pub fn block(&self, coords: Point3<i64>) -> Block {
        self.get(World::chunk_coords(coords))
            .map_or(Block::Air, |chunk| chunk[World::block_coords(coords)])
    }

    fn get(&self, coords: Point3<i32>) -> Option<&Chunk> {
        self.cells.get(&coords).map(Deref::deref)
    }

    fn load(&mut self, coords: Point3<i32>) -> bool {
        if let Some(cell) = self.cells.get_mut(&coords) {
            cell.load();
            true
        } else {
            false
        }
    }

    fn unload(&mut self, coords: Point3<i32>) -> bool {
        if let Some(cell) = self.cells.remove(&coords) {
            if let Some(cell) = cell.unload() {
                self.cells.insert(coords, cell);
            } else {
                self.remove_from_ranges(coords);
            }
            true
        } else {
            false
        }
    }

    #[allow(clippy::type_complexity)]
    fn apply(
        &mut self,
        coords: Point3<i64>,
        action: &BlockAction,
    ) -> Result<(Option<Point3<i32>>, Option<Point3<i32>>), ()> {
        let chunk_coords = World::chunk_coords(coords);
        let block_coords = World::block_coords(coords);
        if let Some(cell) = self.cells.remove(&chunk_coords) {
            match cell.apply(block_coords, action) {
                Ok(Some(cell)) => {
                    self.cells.insert(chunk_coords, cell);
                    Ok((None, None))
                }
                Ok(None) => {
                    self.remove_from_ranges(chunk_coords);
                    Ok((None, Some(chunk_coords)))
                }
                Err(cell) => {
                    self.cells.insert(chunk_coords, cell);
                    Err(())
                }
            }
        } else if let Ok(Some(cell)) = ChunkCell::default_with_action(block_coords, action) {
            self.insert(chunk_coords, cell);
            Ok((Some(chunk_coords), None))
        } else {
            Err(())
        }
    }

    fn insert(&mut self, coords: Point3<i32>, cell: ChunkCell) {
        self.cells.insert(coords, cell);
        self.insert_into_ranges(coords);
    }

    fn insert_into_ranges(&mut self, coords: Point3<i32>) {
        self.y_ranges
            .entry(coords.xz())
            .and_modify(|range| *range = range.start.min(coords.y)..range.end.max(coords.y + 1))
            .or_insert(coords.y..coords.y + 1);
    }

    fn remove_from_ranges(&mut self, coords: Point3<i32>) {
        if let Entry::Occupied(mut entry) = self.y_ranges.entry(coords.xz()) {
            let range = entry.get_mut();
            if range.contains(&coords.y) {
                if range.len() == 1 {
                    entry.remove();
                } else if coords.y == range.start {
                    range.start += 1;
                } else if coords.y == range.end - 1 {
                    range.end -= 1;
                }
            } else {
                unreachable!();
            }
        } else {
            unreachable!();
        }
    }
}

impl Index<Point3<i32>> for ChunkStore {
    type Output = Chunk;

    fn index(&self, coords: Point3<i32>) -> &Self::Output {
        &self.cells[&coords]
    }
}

pub struct ChunkCell(Box<Chunk>);

impl ChunkCell {
    fn new(chunk: Chunk) -> Option<Self> {
        chunk.is_not_empty().then(|| Self(Box::new(chunk)))
    }

    fn default_with_action(coords: Point3<u8>, action: &BlockAction) -> Result<Option<Self>, ()> {
        let mut chunk = Chunk::default();
        if chunk.apply(coords, action) {
            Ok(Self::new(chunk))
        } else {
            Err(())
        }
    }

    fn load(&mut self) {}

    fn unload(self) -> Option<Self> {
        None
    }

    fn apply(mut self, coords: Point3<u8>, action: &BlockAction) -> Result<Option<Self>, Self> {
        if self.0.apply(coords, action) {
            Ok(self.0.is_not_empty().then_some(self))
        } else {
            Err(self)
        }
    }
}

impl Deref for ChunkCell {
    type Target = Chunk;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct ChunkData {
    chunk: Chunk,
    area: ChunkArea,
    area_light: ChunkAreaLight,
}

impl ChunkData {
    pub fn vertices(&self) -> impl Iterator<Item = BlockVertex> + '_ {
        self.chunk.vertices(&self.area, &self.area_light)
    }
}

pub enum WorldEvent {
    InitialRenderRequested {
        area: WorldArea,
        ray: Ray,
    },
    WorldAreaChanged {
        prev: WorldArea,
        curr: WorldArea,
        ray: Ray,
    },
    BlockHoverRequested {
        ray: Ray,
    },
    BlockPlaced {
        block: Block,
        ray: Ray,
    },
    BlockDestroyed {
        ray: Ray,
    },
}

impl WorldEvent {
    pub fn new(
        event: &Event,
        Player {
            prev, curr, ray, ..
        }: &Player,
    ) -> Option<Self> {
        if let Event::ClientEvent(event) = event {
            match event {
                ClientEvent::InitialRenderRequested { .. } => Some(Self::InitialRenderRequested {
                    area: *curr,
                    ray: *ray,
                }),
                ClientEvent::PlayerPositionChanged { .. } if curr != prev => {
                    Some(Self::WorldAreaChanged {
                        prev: *prev,
                        curr: *curr,
                        ray: *ray,
                    })
                }
                ClientEvent::PlayerPositionChanged { .. } => {
                    Some(Self::BlockHoverRequested { ray: *ray })
                }
                ClientEvent::PlayerOrientationChanged { .. } => {
                    Some(Self::BlockHoverRequested { ray: *ray })
                }
                ClientEvent::BlockPlaced { block } => Some(Self::BlockPlaced {
                    block: *block,
                    ray: *ray,
                }),
                ClientEvent::BlockDestroyed => Some(Self::BlockDestroyed { ray: *ray }),
            }
        } else {
            None
        }
    }
}
