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
    client::{event_loop::EventLoopProxy, game::world::BlockVertex, ClientEvent},
    server::{
        event_loop::{Event, EventHandler},
        game::player::{
            ray::{BlockIntersection, Ray},
            Player, WorldArea,
        },
        ServerEvent, SERVER_CONFIG,
    },
    shared::{bound::Aabb, enum_map::Enum, utils},
};
use nalgebra::{Point3, Vector3};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    collections::{hash_map::Entry, LinkedList},
    iter, mem,
    ops::Range,
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

    fn apply(
        &mut self,
        coords: Point3<i64>,
        normal: Vector3<i64>,
        action: BlockAction,
        proxy: &EventLoopProxy,
        ray: Ray,
    ) {
        let mut branch = Branch::default();
        if branch.apply(&self.chunks, coords, normal, action) {
            let (loads, unloads, block_updates) = branch.merge(self);
            let updates = self.updates(
                &loads.iter().chain(&unloads).copied().collect(),
                block_updates,
                false,
            );

            self.handle(&WorldEvent::BlockHoverRequested { ray }, proxy);

            self.send_updates(updates, proxy, true);
            self.send_unloads(unloads, proxy);
            self.send_loads(loads, proxy, true);
        }
    }

    fn send_loads<I>(&self, points: I, proxy: &EventLoopProxy, is_important: bool)
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        self.send_events(
            points.into_iter().map(|coords| ServerEvent::ChunkLoaded {
                coords,
                data: ChunkData::new(&self.chunks, &self.light, coords).into(),
                is_important,
            }),
            proxy,
        );
    }

    fn par_send_loads<I>(&self, points: I, proxy: &EventLoopProxy, is_important: bool)
    where
        I: IntoParallelIterator<Item = Point3<i32>>,
    {
        self.send_events(
            points
                .into_par_iter()
                .map(|coords| ServerEvent::ChunkLoaded {
                    coords,
                    data: ChunkData::new(&self.chunks, &self.light, coords).into(),
                    is_important,
                })
                .collect::<LinkedList<_>>(),
            proxy,
        );
    }

    fn send_unloads<I>(&self, points: I, proxy: &EventLoopProxy)
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        self.send_events(
            points
                .into_iter()
                .map(|coords| ServerEvent::ChunkUnloaded { coords }),
            proxy,
        );
    }

    fn send_updates<I: IntoIterator<Item = Point3<i32>>>(
        &self,
        points: I,
        proxy: &EventLoopProxy,
        is_important: bool,
    ) {
        self.send_events(
            points.into_iter().map(|coords| ServerEvent::ChunkUpdated {
                coords,
                data: ChunkData::new(&self.chunks, &self.light, coords).into(),
                is_important,
            }),
            proxy,
        );
    }

    fn par_send_updates<I: IntoParallelIterator<Item = Point3<i32>>>(
        &self,
        points: I,
        proxy: &EventLoopProxy,
        is_important: bool,
    ) {
        self.send_events(
            points
                .into_par_iter()
                .map(|coords| ServerEvent::ChunkUpdated {
                    coords,
                    data: ChunkData::new(&self.chunks, &self.light, coords).into(),
                    is_important,
                })
                .collect::<LinkedList<_>>(),
            proxy,
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

    fn send_events<I>(&self, events: I, proxy: &EventLoopProxy)
    where
        I: IntoIterator<Item = ServerEvent>,
    {
        for event in events {
            if proxy.send_event(event).is_err() {
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
    type Context<'a> = &'a EventLoopProxy;

    fn handle(&mut self, event: &WorldEvent, proxy: Self::Context<'_>) {
        match *event {
            WorldEvent::InitialRenderRequested { area, ray } => {
                let mut loads = self.par_load_many(area.par_points());

                self.handle(&WorldEvent::BlockHoverRequested { ray }, proxy);

                loads.par_sort_unstable_by_key(|coords| {
                    utils::magnitude_squared(coords - utils::chunk_coords(ray.origin))
                });

                self.par_send_loads(loads, proxy, false);
            }
            WorldEvent::WorldAreaChanged { prev, curr, ray } => {
                let unloads = self.unload_many(prev.exclusive_points(curr));
                let loads = self.par_load_many(curr.par_exclusive_points(prev));

                self.handle(&WorldEvent::BlockHoverRequested { ray }, proxy);

                self.send_unloads(unloads.iter().copied(), proxy);
                self.par_send_loads(loads.par_iter().copied(), proxy, false);
                self.par_send_updates(
                    self.updates(&loads.into_iter().chain(unloads).collect(), [], true),
                    proxy,
                    false,
                );
            }
            WorldEvent::BlockHoverRequested { ray } => {
                let hover = ray.cast(SERVER_CONFIG.player.reach.clone()).find(
                    |&BlockIntersection { coords, .. }| {
                        self.chunks.block(coords).data().hitbox(coords).hit(ray)
                    },
                );

                if mem::replace(&mut self.hover, hover) != hover {
                    _ = proxy.send_event(ServerEvent::BlockHovered(hover.map(
                        |BlockIntersection { coords, .. }| {
                            BlockHoverData::new(
                                coords,
                                self.chunks.block_area(coords),
                                &self.light.block_area_light(coords),
                            )
                        },
                    )));
                }
            }
            WorldEvent::BlockPlaced { block, ray } => {
                if let Some(BlockIntersection { coords, normal }) = self.hover {
                    self.apply(
                        coords + normal,
                        normal,
                        BlockAction::Place(block),
                        proxy,
                        ray,
                    );
                }
            }
            WorldEvent::BlockDestroyed { ray } => {
                if let Some(BlockIntersection { coords, normal }) = self.hover {
                    self.apply(coords, normal, BlockAction::Destroy, proxy, ray);
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

    fn get(&self, coords: Point3<i32>) -> Option<&Chunk> {
        Some(self.0.get(&coords)?)
    }

    fn entry(&mut self, coords: Point3<i32>) -> Entry<Point3<i32>, Box<Chunk>> {
        self.0.entry(coords)
    }

    fn contains(&self, coords: Point3<i32>) -> bool {
        self.0.contains_key(&coords)
    }
}

#[derive(Default)]
struct Branch(FxHashMap<Point3<i32>, FxHashMap<Point3<u8>, BlockAction>>);

impl Branch {
    fn apply(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        normal: Vector3<i64>,
        action: BlockAction,
    ) -> bool {
        if self.is_action_valid(chunks, coords, normal, action) {
            self.insert(coords, action);
            true
        } else {
            false
        }
    }

    #[allow(clippy::type_complexity)]
    fn merge(
        self,
        World {
            chunks,
            light,
            actions,
            ..
        }: &mut World,
    ) -> (Vec<Point3<i32>>, Vec<Point3<i32>>, Vec<Point3<i64>>) {
        let mut hits = vec![];
        let mut loads = vec![];
        let mut unloads = vec![];

        for (chunk_coords, actions) in self.0 {
            match chunks.entry(chunk_coords) {
                Entry::Occupied(mut entry) => {
                    let chunk = entry.get_mut();
                    for (block_coords, action) in actions {
                        if chunk.apply(block_coords, action) {
                            hits.push((utils::coords((chunk_coords, block_coords)), action));
                        }
                    }
                    if chunk.is_empty() {
                        entry.remove();
                        unloads.push(chunk_coords);
                    }
                }
                Entry::Vacant(entry) => {
                    let mut actions = actions
                        .into_iter()
                        .filter(|&(_, action)| Block::Air.is_action_valid(action))
                        .peekable();

                    if actions.peek().is_some() {
                        let chunk = entry.insert(Default::default());
                        for (block_coords, action) in actions {
                            chunk.apply_unchecked(block_coords, action);
                            hits.push((utils::coords((chunk_coords, block_coords)), action));
                        }
                        loads.push(chunk_coords);
                    }
                }
            }
        }

        (
            loads,
            unloads,
            hits.into_iter()
                .inspect(|&(coords, action)| actions.insert(coords, action))
                .flat_map(|(coords, action)| {
                    iter::once(coords).chain(light.apply(chunks, coords, action))
                })
                .collect(),
        )
    }

    fn is_action_valid(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        normal: Vector3<i64>,
        action: BlockAction,
    ) -> bool {
        if World::Y_RANGE.contains(&utils::chunk_coords(coords).y) {
            match action {
                BlockAction::Place(block) => {
                    if let Some(surface) = block.data().valid_surface {
                        normal == Vector3::y() && chunks.block(coords - normal) == surface
                    } else {
                        true
                    }
                }
                BlockAction::Destroy => {
                    let top = coords + Vector3::y();
                    if chunks.block(top).data().valid_surface.is_some() {
                        self.insert(top, BlockAction::Destroy);
                    }
                    true
                }
            }
        } else {
            false
        }
    }

    fn insert(&mut self, coords: Point3<i64>, action: BlockAction) {
        self.0
            .entry(utils::chunk_coords(coords))
            .or_default()
            .entry(utils::block_coords(coords))
            .and_modify(|_| unreachable!())
            .or_insert(action);
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
    ) -> impl Iterator<Item = (BlockData, impl Iterator<Item = BlockVertex>)> + '_ {
        self.blocks().map(|(coords, block)| {
            let data = block.data();
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

    fn blocks(&self) -> impl Iterator<Item = (Point3<u8>, Block)> + '_ {
        Chunk::points()
            .map(|coords| (coords, self.area.block(coords)))
            .filter(|(_, block)| *block != Block::Air)
    }
}

#[derive(Clone, Copy)]
pub struct BlockHoverData {
    pub hitbox: Aabb,
    pub brightness: BlockLight,
}

impl BlockHoverData {
    fn new(coords: Point3<i64>, area: BlockArea, area_light: &BlockAreaLight) -> Self {
        let data = area.block().data();
        Self {
            hitbox: data.hitbox(coords),
            brightness: Self::brightness(data, area, area_light),
        }
    }

    fn brightness(data: BlockData, area: BlockArea, area_light: &BlockAreaLight) -> BlockLight {
        let is_externally_lit = data.is_externally_lit();
        Option::variants()
            .flat_map(|side| {
                area_light
                    .corner_lights(side, area, is_externally_lit)
                    .into_values()
            })
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
    pub fn new(event: &Event, &Player { prev, curr, ray }: &Player) -> Option<Self> {
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
