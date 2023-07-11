pub mod action;
pub mod block;
pub mod chunk;
pub mod light;

use self::{
    action::{ActionStore, BlockAction},
    block::{
        area::{BlockArea, BlockAreaLight},
        data::BlockData,
        Block, BlockLight,
    },
    chunk::{
        area::{ChunkArea, ChunkAreaLight},
        generator::ChunkGenerator,
        Chunk,
    },
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
use enum_map::enum_map;
use flume::Sender;
use nalgebra::Point3;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    cmp,
    collections::LinkedList,
    ops::{Deref, Range},
    sync::Arc,
};

#[derive(Default)]
pub struct World {
    chunks: ChunkStore,
    generator: ChunkGenerator,
    actions: ActionStore,
    light: WorldLight,
    hover: Option<BlockIntersection>,
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
            chunk.apply_unchecked(coords, action);
        }
        ChunkCell::new(chunk)
    }

    fn load_event(&self, coords: Point3<i32>, is_important: bool) -> ServerEvent {
        ServerEvent::ChunkLoaded {
            coords,
            data: Arc::new(ChunkData {
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
            .map(utils::chunk_coords)
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
                let hover =
                    ray.cast(self.reach.clone())
                        .find(|BlockIntersection { coords, .. }| {
                            self.chunks.block(*coords) != Block::Air
                        });

                if self.hover != hover {
                    self.hover = hover;
                    server_tx
                        .send(ServerEvent::BlockHovered(hover.map(
                            |BlockIntersection { coords, .. }| {
                                BlockHoverData::new(
                                    coords,
                                    self.chunks.block_area(coords),
                                    &self.light.block_area_light(coords),
                                )
                            },
                        )))
                        .unwrap_or_else(|_| unreachable!());
                }
            }
            WorldEvent::BlockPlaced { block, ray } => {
                if let Some(BlockIntersection { coords, normal }) = self.hover {
                    self.apply(coords + normal, BlockAction::Place(*block), server_tx, *ray);
                }
            }
            WorldEvent::BlockDestroyed { ray } => {
                if let Some(BlockIntersection { coords, .. }) = self.hover {
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
        for delta in ChunkArea::chunk_deltas() {
            if let Some(light) = self.0.get(&(coords + delta)) {
                for (coords, delta) in ChunkArea::block_deltas(delta) {
                    value[delta] = light[coords];
                }
            }
        }
        value
    }

    fn block_area(&self, coords: Point3<i64>) -> BlockArea {
        BlockArea::from_fn(|delta| self.block(coords + delta.cast()))
    }

    fn block(&self, coords: Point3<i64>) -> Block {
        self.get(utils::chunk_coords(coords))
            .map_or(Block::Air, |chunk| chunk[utils::block_coords(coords)])
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
        let chunk_coords = utils::chunk_coords(coords);
        let block_coords = utils::block_coords(coords);
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
    area: ChunkArea,
    area_light: ChunkAreaLight,
}

impl ChunkData {
    pub fn vertices(
        &self,
    ) -> impl Iterator<Item = (&'static BlockData, impl Iterator<Item = BlockVertex>)> + '_ {
        Chunk::points().map(|coords| {
            let data = self.area.block(coords).data();
            (
                data,
                data.vertices(
                    coords,
                    self.area.block_area(coords),
                    self.area_light.block_area_light(coords),
                ),
            )
        })
    }
}

#[derive(Clone, Copy)]
pub struct BlockHoverData {
    pub coords: Point3<i64>,
    pub brightness: BlockLight,
}

impl BlockHoverData {
    fn new(coords: Point3<i64>, area: BlockArea, area_light: &BlockAreaLight) -> Self {
        Self {
            coords,
            brightness: Self::brightness(area, area_light),
        }
    }

    fn brightness(area: BlockArea, area_light: &BlockAreaLight) -> BlockLight {
        let is_smoothly_lit = area.block().data().is_smoothly_lit();
        enum_map! {
            side => area_light.corner_lights(side, area, is_smoothly_lit),
        }
        .into_values()
        .flat_map(|corner_lights| corner_lights.into_values())
        .fold(Default::default(), |accum, light| {
            cmp::max_by(accum, light, |a, b| a.lum().total_cmp(&b.lum()))
        })
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
