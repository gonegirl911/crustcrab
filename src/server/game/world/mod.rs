pub mod action;
pub mod block;
pub mod chunk;
pub mod light;

use self::{
    action::{ActionStore, BlockAction},
    block::{Block, BlockArea},
    chunk::{generator::ChunkGenerator, light::ChunkAreaLight, Chunk, ChunkArea},
    light::WorldLight,
};
use crate::{
    client::{game::world::BlockVertex, ClientEvent},
    server::{
        event_loop::{Event, EventHandler},
        game::player::{
            ray::{BlockIntersection, Ray},
            Player, WorldArea,
        },
        ServerEvent, ServerState,
    },
    shared::utils,
};
use flume::Sender;
use nalgebra::{Point, Point3};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    collections::LinkedList,
    ops::{Deref, Index, Range},
    sync::Arc,
};

#[derive(Default)]
pub struct World {
    chunks: ChunkStore,
    generator: ChunkGenerator,
    actions: ActionStore,
    light: WorldLight,
    hovered_block: Option<BlockIntersection>,
    reach: Range<f32>,
}

impl World {
    const Y_RANGE: Range<i32> = -4..20;

    pub fn new(state: &ServerState) -> Self {
        Self {
            reach: state.reach.clone(),
            ..Default::default()
        }
    }

    fn load_many<I>(&mut self, points: I) -> Vec<Result<Point3<i32>, Point3<i32>>>
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
                Ok(coords) => Some(Ok(coords)),
                Err(Some((coords, cell))) => {
                    self.chunks.insert(coords, cell);
                    Some(Err(coords))
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
        let updates = Self::updates(
            &load.into_iter().chain(unload).collect(),
            self.light
                .apply(&self.chunks, coords, &action)
                .into_iter()
                .chain([coords]),
            false,
        );

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

    fn updates<I: IntoIterator<Item = Point3<i64>>>(
        points: &FxHashSet<Point3<i32>>,
        block_updates: I,
        include_outline: bool,
    ) -> FxHashSet<Point3<i32>> {
        Self::block_area_points(block_updates)
            .map(Self::chunk_coords)
            .chain(
                include_outline
                    .then_some(Self::chunk_area_points(points.iter().copied()))
                    .into_iter()
                    .flatten(),
            )
            .filter(|coords| !points.contains(coords))
            .collect()
    }

    fn chunk_area_points<I>(points: I) -> impl Iterator<Item = Point3<i32>>
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        points
            .into_iter()
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

    fn unzip<T, I>(iter: I) -> (Vec<T>, Vec<T>)
    where
        T: Copy,
        I: IntoIterator<Item = Result<T, T>>,
    {
        let mut all = vec![];
        let mut err = vec![];
        for value in iter {
            match value {
                Ok(value) => all.push(value),
                Err(value) => {
                    all.push(value);
                    err.push(value);
                }
            }
        }
        (all, err)
    }

    fn chunk_coords<const D: usize>(coords: Point<i64, D>) -> Point<i32, D> {
        coords.map(|c| utils::div_floor(c, Chunk::DIM as i64) as i32)
    }

    fn block_coords<const D: usize>(coords: Point<i64, D>) -> Point<u8, D> {
        coords.map(|c| c.rem_euclid(Chunk::DIM as i64) as u8)
    }

    fn coords<const D: usize>(
        chunk_coords: Point<i32, D>,
        block_coords: Point<u8, D>,
    ) -> Point<i64, D> {
        chunk_coords.cast() * Chunk::DIM as i64 + block_coords.coords.cast()
    }
}

impl EventHandler<WorldEvent> for World {
    type Context<'a> = Sender<ServerEvent>;

    fn handle(&mut self, event: &WorldEvent, server_tx: Self::Context<'_>) {
        match event {
            WorldEvent::InitialRenderRequested { area, ray } => {
                let (mut loads, inserts) = Self::unzip(self.load_many(area.points()));
                let _ = self.light.insert_many(&self.chunks, inserts);

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
                let (loads, inserts) = Self::unzip(self.load_many(curr.exclusive_points(prev)));
                let updates = Self::updates(
                    &loads.iter().chain(&unloads).copied().collect(),
                    self.light
                        .remove_many(&self.chunks, unloads.iter().copied())
                        .into_iter()
                        .chain(self.light.insert_many(&self.chunks, inserts)),
                    true,
                );

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
pub struct ChunkStore(FxHashMap<Point3<i32>, ChunkCell>);

impl ChunkStore {
    fn chunk_area(&self, coords: Point3<i32>) -> ChunkArea {
        let mut value = ChunkArea::default();
        for (chunk_delta, deltas) in ChunkArea::deltas() {
            if let Some(chunk) = self.get(coords + chunk_delta) {
                for (block_coords, delta) in deltas {
                    value.set(delta, chunk[block_coords].data().is_opaque());
                }
            }
        }
        value
    }

    fn block(&self, coords: Point3<i64>) -> Block {
        self.get(World::chunk_coords(coords))
            .map_or(Block::Air, |chunk| chunk[World::block_coords(coords)])
    }

    fn get(&self, coords: Point3<i32>) -> Option<&Chunk> {
        self.0.get(&coords).map(Deref::deref)
    }

    fn load(&mut self, coords: Point3<i32>) -> bool {
        if let Some(cell) = self.0.get_mut(&coords) {
            cell.load();
            true
        } else {
            false
        }
    }

    fn unload(&mut self, coords: Point3<i32>) -> bool {
        if let Some(cell) = self.0.remove(&coords) {
            if let Some(cell) = cell.unload() {
                self.insert(coords, cell);
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
        if World::Y_RANGE.contains(&chunk_coords.y) {
            if let Some(cell) = self.0.remove(&chunk_coords) {
                match cell.apply(block_coords, action) {
                    Ok(Some(cell)) => {
                        self.insert(chunk_coords, cell);
                        Ok((None, None))
                    }
                    Ok(None) => Ok((None, Some(chunk_coords))),
                    Err(cell) => {
                        self.insert(chunk_coords, cell);
                        Err(())
                    }
                }
            } else if let Ok(Some(cell)) = ChunkCell::default_with_action(block_coords, action) {
                self.insert(chunk_coords, cell);
                Ok((Some(chunk_coords), None))
            } else {
                Err(())
            }
        } else {
            Err(())
        }
    }

    fn insert(&mut self, coords: Point3<i32>, cell: ChunkCell) {
        self.0.insert(coords, cell);
    }
}

impl Index<Point3<i32>> for ChunkStore {
    type Output = Chunk;

    fn index(&self, coords: Point3<i32>) -> &Self::Output {
        &self.0[&coords]
    }
}

struct ChunkCell(Box<Chunk>);

impl ChunkCell {
    fn new(chunk: Chunk) -> Option<Self> {
        (!chunk.is_empty()).then(|| Self(Box::new(chunk)))
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
            Ok((!self.0.is_empty()).then_some(self))
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
