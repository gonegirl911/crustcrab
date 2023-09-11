pub mod action;
pub mod block;
pub mod chunk;

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
};
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
    shared::{dash::FxDashMap, utils},
};
use dashmap::mapref::entry::Entry;
use enum_map::enum_map;
use flume::Sender;
use nalgebra::Point3;
use rustc_hash::FxHashSet;
use std::{
    cmp,
    ops::{Deref, Range},
    sync::Arc,
};

#[derive(Default)]
pub struct World {
    chunks: Arc<ChunkStore>,
    actions: Arc<ActionStore>,
    hover: Option<BlockIntersection>,
}

impl World {
    pub const Y_RANGE: Range<i32> = -4..20;

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
        let Ok((load, unload)) = self.chunks.apply(coords, &action) else {
            return;
        };
        let updates = Self::updates(&load.into_iter().chain(unload).collect(), [coords], false);

        self.handle(&WorldEvent::BlockHoverRequested { ray }, server_tx.clone());

        self.send_unloads(unload, server_tx.clone());
        self.send_loads(load, server_tx.clone(), true);
        self.send_updates(updates, server_tx, true);

        self.actions.insert(coords, action);
    }

    fn send_loads<I>(&self, points: I, server_tx: Sender<ServerEvent>, is_important: bool)
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        self.send_events(
            points.into_iter().map(|coords| ServerEvent::ChunkLoaded {
                coords,
                data: Arc::new(ChunkData::new(&self.chunks, coords)),
                is_important,
            }),
            server_tx,
        );
    }

    fn send_unloads<I>(&self, points: I, server_tx: Sender<ServerEvent>)
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
        server_tx: Sender<ServerEvent>,
        is_important: bool,
    ) {
        self.send_events(
            points.into_iter().map(|coords| ServerEvent::ChunkUpdated {
                coords,
                data: Arc::new(ChunkData::new(&self.chunks, coords)),
                is_important,
            }),
            server_tx,
        );
    }

    fn send_events<I>(&self, events: I, server_tx: Sender<ServerEvent>)
    where
        I: IntoIterator<Item = ServerEvent>,
    {
        for event in events {
            server_tx.send(event).unwrap_or_else(|_| unreachable!());
        }
    }

    fn gen(generator: &ChunkGenerator, actions: &ActionStore, coords: Point3<i32>) -> Chunk {
        let mut chunk = generator.gen(coords);
        actions.apply_unchecked(coords, &mut chunk);
        chunk
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
}

impl EventHandler<WorldEvent> for World {
    type Context<'a> = Sender<ServerEvent>;

    fn handle(&mut self, event: &WorldEvent, server_tx: Self::Context<'_>) {
        match event {
            WorldEvent::InitialRenderRequested { area } => {}
            WorldEvent::WorldAreaChanged { prev, curr, ray } => {
                let unloads = self.unload_many(prev.exclusive_points(curr));

                self.handle(
                    &WorldEvent::BlockHoverRequested { ray: *ray },
                    server_tx.clone(),
                );

                self.send_unloads(unloads, server_tx.clone());
            }
            WorldEvent::BlockHoverRequested { ray } => {
                let hover = ray.cast(SERVER_CONFIG.player.reach.clone()).find(
                    |BlockIntersection { coords, .. }| self.chunks.block(*coords) != Block::Air,
                );

                if self.hover != hover {
                    self.hover = hover;
                    server_tx
                        .send(ServerEvent::BlockHovered(hover.map(
                            |BlockIntersection { coords, .. }| {
                                BlockHoverData::new(
                                    coords,
                                    self.chunks.block_area(coords),
                                    &Default::default(),
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
            WorldEvent::Tick { ray } => {}
        }
    }
}

#[derive(Default)]
pub struct ChunkStore(FxDashMap<Point3<i32>, Chunk>);

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

    fn get(&self, coords: Point3<i32>) -> Option<impl Deref<Target = Chunk> + '_> {
        self.0.get(&coords)
    }

    fn load(&self, coords: Point3<i32>, chunk: Chunk) -> bool {
        if !chunk.is_empty() {
            self.0.insert(coords, chunk);
            true
        } else {
            false
        }
    }

    fn unload(&self, coords: Point3<i32>) -> bool {
        self.0.remove(&coords).is_some()
    }

    #[allow(clippy::type_complexity)]
    fn apply(
        &self,
        coords: Point3<i64>,
        action: &BlockAction,
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
                    let mut chunk = Chunk::default();
                    if chunk.apply(block_coords, action) {
                        if !chunk.is_empty() {
                            entry.insert(chunk);
                            Ok((Some(chunk_coords), None))
                        } else {
                            unreachable!();
                        }
                    } else {
                        Err(())
                    }
                }
            }
        } else {
            Err(())
        }
    }
}

pub struct ChunkData {
    area: ChunkArea,
    area_light: ChunkAreaLight,
}

impl ChunkData {
    fn new(chunks: &ChunkStore, coords: Point3<i32>) -> Self {
        Self {
            area: chunks.chunk_area(coords),
            area_light: Default::default(),
        }
    }

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
    Tick {
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
        match event {
            Event::ClientEvent(ClientEvent::InitialRenderRequested { .. }) => {
                Some(Self::InitialRenderRequested { area: *curr })
            }
            Event::ClientEvent(ClientEvent::PlayerPositionChanged { .. }) if curr != prev => {
                Some(Self::WorldAreaChanged {
                    prev: *prev,
                    curr: *curr,
                    ray: *ray,
                })
            }
            Event::ClientEvent(ClientEvent::PlayerPositionChanged { .. }) => {
                Some(Self::BlockHoverRequested { ray: *ray })
            }
            Event::ClientEvent(ClientEvent::PlayerOrientationChanged { .. }) => {
                Some(Self::BlockHoverRequested { ray: *ray })
            }
            Event::ClientEvent(ClientEvent::BlockPlaced { block }) => Some(Self::BlockPlaced {
                block: *block,
                ray: *ray,
            }),
            Event::ClientEvent(ClientEvent::BlockDestroyed) => {
                Some(Self::BlockDestroyed { ray: *ray })
            }
            Event::Tick => Some(Self::Tick { ray: *ray }),
            _ => None,
        }
    }
}
