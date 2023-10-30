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
use super::player::ray::Hittable;
use crate::{
    client::{game::world::BlockVertex, ClientEvent},
    server::{
        event_loop::{Event, EventHandler},
        game::player::{
            ray::{BlockIntersection, Ray},
            Player, WorldArea,
        },
        ServerEvent, SERVER_CONFIG,
    },
    shared::{bound::Aabb, utils},
};
use enum_map::enum_map;
use flume::Sender;
use nalgebra::Point3;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    collections::{hash_map::Entry, LinkedList},
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
}

impl World {
    pub const Y_RANGE: Range<i32> = -4..20;

    fn par_load_many<I>(&mut self, points: I) -> Vec<Point3<i32>>
    where
        I: IntoParallelIterator<Item = Point3<i32>>,
    {
        points
            .into_par_iter()
            .map(|coords| (coords, self.gen(coords)))
            .collect::<LinkedList<_>>()
            .into_iter()
            .filter_map(|(coords, chunk)| self.chunks.load(coords, chunk).then_some(coords))
            .collect()
    }

    fn unload_many<I>(&mut self, points: I) -> Vec<Point3<i32>>
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        points
            .into_iter()
            .filter(|&coords| self.chunks.unload(coords))
            .collect()
    }

    #[rustfmt::skip]
    fn apply(
        &mut self,
        coords: Point3<i64>,
        action: BlockAction,
        server_tx: &Sender<ServerEvent>,
        ray: Ray,
    ) {
        let Ok((load, unload)) = self.chunks.apply(coords, action) else { return };
        let light_updates = self.light.apply(&self.chunks, coords, action);

        self.handle(&WorldEvent::BlockHoverRequested { ray }, server_tx);

        self.actions.insert(coords, action);

        self.send_unloads(unload, server_tx);
        self.send_loads(load, server_tx, true);
        self.send_updates(
            self.updates(
                &load.into_iter().chain(unload).collect(),
                light_updates.into_iter().chain([coords]),
                false,
            ),
            server_tx,
            true,
        );
    }

    fn send_loads<I>(&self, points: I, server_tx: &Sender<ServerEvent>, is_important: bool)
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        self.send_events(
            points.into_iter().map(|coords| ServerEvent::ChunkLoaded {
                coords,
                data: Arc::new(ChunkData::new(&self.chunks, &self.light, coords)),
                is_important,
            }),
            server_tx,
        );
    }

    fn par_send_loads<I>(&self, points: I, server_tx: &Sender<ServerEvent>, is_important: bool)
    where
        I: IntoParallelIterator<Item = Point3<i32>>,
    {
        self.send_events(
            points
                .into_par_iter()
                .map(|coords| ServerEvent::ChunkLoaded {
                    coords,
                    data: Arc::new(ChunkData::new(&self.chunks, &self.light, coords)),
                    is_important,
                })
                .collect::<LinkedList<_>>(),
            server_tx,
        );
    }

    fn send_unloads<I>(&self, points: I, server_tx: &Sender<ServerEvent>)
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        self.send_events(
            points
                .into_iter()
                .map(|coords| ServerEvent::ChunkUnloaded { coords }),
            server_tx,
        );
    }

    fn send_updates<I: IntoIterator<Item = Point3<i32>>>(
        &self,
        points: I,
        server_tx: &Sender<ServerEvent>,
        is_important: bool,
    ) {
        self.send_events(
            points.into_iter().map(|coords| ServerEvent::ChunkUpdated {
                coords,
                data: Arc::new(ChunkData::new(&self.chunks, &self.light, coords)),
                is_important,
            }),
            server_tx,
        );
    }

    fn par_send_updates<I: IntoParallelIterator<Item = Point3<i32>>>(
        &self,
        points: I,
        server_tx: &Sender<ServerEvent>,
        is_important: bool,
    ) {
        self.send_events(
            points
                .into_par_iter()
                .map(|coords| ServerEvent::ChunkUpdated {
                    coords,
                    data: Arc::new(ChunkData::new(&self.chunks, &self.light, coords)),
                    is_important,
                })
                .collect::<LinkedList<_>>(),
            server_tx,
        );
    }

    fn gen(&self, coords: Point3<i32>) -> Box<Chunk> {
        let mut chunk = Box::new(self.generator.gen(coords));
        for (coords, action) in self.actions.actions(coords) {
            chunk.apply_unchecked(coords, action);
        }
        chunk
    }

    fn updates<I: IntoIterator<Item = Point3<i64>>>(
        &self,
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
            .filter(|coords| self.chunks.contains(*coords) && !points.contains(coords))
            .collect()
    }

    fn send_events<I>(&self, events: I, server_tx: &Sender<ServerEvent>)
    where
        I: IntoIterator<Item = ServerEvent>,
    {
        for event in events {
            if server_tx.send(event).is_err() {
                break;
            }
        }
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
}

impl EventHandler<WorldEvent> for World {
    type Context<'a> = &'a Sender<ServerEvent>;

    fn handle(&mut self, event: &WorldEvent, server_tx: Self::Context<'_>) {
        match *event {
            WorldEvent::InitialRenderRequested { area, ray } => {
                let mut loads = self.par_load_many(area.par_points());

                self.handle(&WorldEvent::BlockHoverRequested { ray }, server_tx);

                loads.par_sort_unstable_by_key(|coords| {
                    utils::magnitude_squared(coords - utils::chunk_coords(ray.origin))
                });

                self.par_send_loads(loads, server_tx, false);
            }
            WorldEvent::WorldAreaChanged { prev, curr, ray } => {
                let unloads = self.unload_many(prev.exclusive_points(curr));
                let loads = self.par_load_many(curr.par_exclusive_points(prev));

                self.handle(&WorldEvent::BlockHoverRequested { ray }, server_tx);

                self.send_unloads(unloads.iter().copied(), server_tx);
                self.par_send_loads(loads.par_iter().copied(), server_tx, false);
                self.par_send_updates(
                    self.updates(&unloads.into_iter().chain(loads).collect(), [], true),
                    server_tx,
                    false,
                );
            }
            WorldEvent::BlockHoverRequested { ray } => {
                let hover = ray.cast(SERVER_CONFIG.player.reach.clone()).find(
                    |&BlockIntersection { coords, .. }| {
                        self.chunks.block(coords).data().hitbox(coords).hit(ray)
                    },
                );

                if self.hover != hover {
                    self.hover = hover;
                    server_tx
                        .send(ServerEvent::BlockHovered(hover.map(
                            |BlockIntersection { coords, .. }| {
                                BlockHoverData::new(
                                    coords,
                                    &self.chunks.block_area(coords),
                                    &self.light.block_area_light(coords),
                                )
                            },
                        )))
                        .unwrap_or_else(|_| unreachable!());
                }
            }
            WorldEvent::BlockPlaced { block, ray } => {
                if let Some(BlockIntersection { coords, normal }) = self.hover {
                    self.apply(coords + normal, BlockAction::Place(block), server_tx, ray);
                }
            }
            WorldEvent::BlockDestroyed { ray } => {
                if let Some(BlockIntersection { coords, .. }) = self.hover {
                    self.apply(coords, BlockAction::Destroy, server_tx, ray);
                }
            }
        }
    }
}

#[derive(Default)]
pub struct ChunkStore(FxHashMap<Point3<i32>, Box<Chunk>>);

impl ChunkStore {
    fn chunk_area(&self, coords: Point3<i32>) -> ChunkArea {
        let mut value = ChunkArea::default();
        for delta in ChunkArea::chunk_deltas() {
            if let Some(chunk) = self.get(coords + delta) {
                for (coords, delta) in ChunkArea::block_deltas(delta) {
                    value[delta] = chunk[coords];
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

    fn load(&mut self, coords: Point3<i32>, chunk: Box<Chunk>) -> bool {
        if !chunk.is_empty() {
            assert!(self.0.insert(coords, chunk).is_none());
            true
        } else {
            false
        }
    }

    fn unload(&mut self, coords: Point3<i32>) -> bool {
        self.0.remove(&coords).is_some()
    }

    #[allow(clippy::type_complexity)]
    fn apply(
        &mut self,
        coords: Point3<i64>,
        action: BlockAction,
    ) -> Result<(Option<Point3<i32>>, Option<Point3<i32>>), ()> {
        let chunk_coords = utils::chunk_coords(coords);
        let block_coords = utils::block_coords(coords);
        if World::Y_RANGE.contains(&chunk_coords.y) {
            match self.0.entry(chunk_coords) {
                Entry::Occupied(mut entry) => {
                    let chunk = entry.get_mut();
                    if chunk.apply(block_coords, action) {
                        if !chunk.is_empty() {
                            Ok((None, None))
                        } else {
                            entry.remove();
                            Ok((None, Some(chunk_coords)))
                        }
                    } else {
                        Err(())
                    }
                }
                Entry::Vacant(entry) => {
                    if Block::Air.apply(action) {
                        let chunk = entry.insert(Default::default());
                        chunk.apply_unchecked(block_coords, action);
                        Ok((Some(chunk_coords), None))
                    } else {
                        Err(())
                    }
                }
            }
        } else {
            Err(())
        }
    }

    fn get(&self, coords: Point3<i32>) -> Option<&Chunk> {
        self.0.get(&coords).map(Deref::deref)
    }

    fn contains(&self, coords: Point3<i32>) -> bool {
        self.0.contains_key(&coords)
    }
}

pub struct ChunkData {
    area: ChunkArea,
    area_light: ChunkAreaLight,
}

impl ChunkData {
    fn new(chunks: &ChunkStore, light: &WorldLight, coords: Point3<i32>) -> Self {
        Self {
            area: chunks.chunk_area(coords),
            area_light: light.chunk_area_light(coords),
        }
    }

    pub fn vertices(
        &self,
    ) -> impl Iterator<Item = (&'static BlockData, impl Iterator<Item = BlockVertex>)> + '_ {
        Chunk::points().filter_map(|coords| {
            let data = self.area.block(coords).data();
            Some((
                data,
                data.vertices(
                    coords,
                    self.area.block_area(coords),
                    self.area_light.block_area_light(coords),
                )?,
            ))
        })
    }
}

#[derive(Clone, Copy)]
pub struct BlockHoverData {
    pub hitbox: Aabb,
    pub brightness: BlockLight,
}

impl BlockHoverData {
    fn new(coords: Point3<i64>, area: &BlockArea, area_light: &BlockAreaLight) -> Self {
        let data = area.block().data();
        Self {
            hitbox: data.hitbox(coords),
            brightness: Self::brightness(data, area, area_light),
        }
    }

    fn brightness(data: &BlockData, area: &BlockArea, area_light: &BlockAreaLight) -> BlockLight {
        enum_map! {
            side => area_light.corner_lights(side, area, data.is_externally_lit()),
        }
        .into_values()
        .flat_map(|corner_lights| corner_lights.into_values())
        .max_by(|a, b| a.lum().total_cmp(&b.lum()))
        .unwrap_or_else(|| unreachable!())
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
        &Player {
            prev, curr, ray, ..
        }: &Player,
    ) -> Option<Self> {
        match *event {
            Event::ClientEvent(ClientEvent::InitialRenderRequested { .. }) => {
                Some(Self::InitialRenderRequested { area: curr, ray })
            }
            Event::ClientEvent(ClientEvent::PlayerPositionChanged { .. }) if curr != prev => {
                Some(Self::WorldAreaChanged { prev, curr, ray })
            }
            Event::ClientEvent(ClientEvent::PlayerPositionChanged { .. }) => {
                Some(Self::BlockHoverRequested { ray })
            }
            Event::ClientEvent(ClientEvent::PlayerOrientationChanged { .. }) => {
                Some(Self::BlockHoverRequested { ray })
            }
            Event::ClientEvent(ClientEvent::BlockPlaced { block }) => {
                Some(Self::BlockPlaced { block, ray })
            }
            Event::ClientEvent(ClientEvent::BlockDestroyed) => Some(Self::BlockDestroyed { ray }),
            _ => None,
        }
    }
}
