pub mod action;
pub mod block;
pub mod chunk;
pub mod height;
pub mod light;

use self::{
    action::{ActionStore, BlockAction},
    block::{
        area::{BlockArea, BlockAreaLight},
        data::{Corner, Side, SIDE_DELTAS, SIDE_MASKS},
        Block, BlockLight,
    },
    chunk::{
        area::{ChunkArea, ChunkAreaLight},
        generator::ChunkGenerator,
        Chunk, DataStore,
    },
    height::HeightMap,
    light::WorldLight,
};
use super::player::{Player, WorldArea};
use crate::{
    client::{event_loop::EventLoopProxy, game::world::BlockVertex, ClientEvent},
    server::{
        event_loop::{Event, EventHandler},
        GroupId, ServerEvent, SERVER_CONFIG,
    },
    shared::{
        bound::Aabb,
        enum_map::{Enum, EnumMap},
        ray::{BlockIntersection, Intersectable, Ray},
        utils,
        utils::IntoSequentialIteratorExt,
    },
};
use nalgebra::{point, Point2, Point3, Vector3};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    array,
    collections::hash_map::Entry,
    iter, mem,
    ops::{Index, Range},
};

#[derive(Default)]
pub struct World {
    chunks: ChunkStore,
    heights: HeightMap,
    generator: ChunkGenerator,
    actions: ActionStore,
    light: WorldLight,
    hover: Option<BlockIntersection>,
}

impl World {
    pub const Y_RANGE: Range<i32> = -4..20;

    fn par_insert_many<P>(&mut self, points: P) -> Vec<Point3<i32>>
    where
        P: IntoParallelIterator<Item = Point3<i32>>,
    {
        points
            .into_par_iter()
            .filter_map(|coords| Some((coords, self.generate(coords)?)))
            .into_seq_iter()
            .map(|(coords, chunk)| {
                self.chunks.insert(coords, chunk);
                coords
            })
            .collect()
    }

    #[rustfmt::skip]
    fn par_light_up(&mut self, points: &[Point3<i32>]) -> Vec<Point3<i64>> {
        self.light.extend_placeholders(self.heights.load_placeholders(points));
        self.light.par_insert_many(&self.chunks, &self.heights, points)
    }

    fn exclusive_points(
        &self,
        prev: WorldArea,
        curr: WorldArea,
    ) -> impl Iterator<Item = Point3<i32>> + '_ {
        curr.points().filter(move |&coords| {
            curr.contains_y(coords.y) && !prev.contains(coords) && self.chunks.contains(coords)
        })
    }

    fn updates(
        &self,
        inserts: impl IntoIterator<Item = Point3<i32>>,
        block_updates: impl IntoIterator<Item = Point3<i64>>,
        loads: &FxHashSet<Point3<i32>>,
        unloads: &FxHashSet<Point3<i32>>,
    ) -> FxHashSet<Point3<i32>> {
        Self::chunk_area_points(inserts)
            .chain(Self::block_area_points(block_updates).map(utils::chunk_coords))
            .filter(|coords| {
                self.chunks.contains(*coords)
                    && !loads.contains(coords)
                    && !unloads.contains(coords)
            })
            .collect()
    }

    fn apply(
        &mut self,
        coords: Point3<i64>,
        normal: Vector3<i64>,
        action: BlockAction,
        proxy: &EventLoopProxy,
        area: WorldArea,
        ray: Ray,
    ) {
        let mut branch = Branch::default();
        if branch.apply(&self.chunks, coords, normal, action) {
            let (block_updates, inserts, removals) = branch.merge(self);
            let updates = self.updates([], block_updates, &inserts, &removals);
            let group_id = GroupId::new(
                Self::filter(
                    inserts.iter().chain(&updates).chain(&removals).copied(),
                    area,
                )
                .count(),
            );

            self.handle(&WorldEvent::BlockHoverRequested { ray }, proxy);

            self.send_updates(Self::filter(updates, area), group_id, proxy);
            Self::send_unloads(Self::filter(removals, area), Some(group_id), proxy);
            self.send_loads(Self::filter(inserts, area), group_id, proxy);
        }
    }

    fn send_loads<P>(&self, points: P, group_id: GroupId, proxy: &EventLoopProxy)
    where
        P: IntoIterator<Item = Point3<i32>>,
    {
        Self::send_events(
            points.into_iter().filter_map(|coords| {
                Some(ServerEvent::ChunkLoaded {
                    coords,
                    data: ChunkData::new(&self.chunks, &self.light, coords)?.into(),
                    group_id: Some(group_id),
                })
            }),
            proxy,
        );
    }

    fn par_send_loads<P>(&self, points: P, proxy: &EventLoopProxy)
    where
        P: IntoParallelIterator<Item = Point3<i32>>,
    {
        Self::send_events(
            points
                .into_par_iter()
                .filter_map(|coords| {
                    Some(ServerEvent::ChunkLoaded {
                        coords,
                        data: ChunkData::new(&self.chunks, &self.light, coords)?.into(),
                        group_id: None,
                    })
                })
                .into_seq_iter(),
            proxy,
        );
    }

    fn send_updates<P>(&self, points: P, group_id: GroupId, proxy: &EventLoopProxy)
    where
        P: IntoIterator<Item = Point3<i32>>,
    {
        Self::send_events(
            points.into_iter().filter_map(|coords| {
                Some(ServerEvent::ChunkUpdated {
                    coords,
                    data: ChunkData::new(&self.chunks, &self.light, coords)?.into(),
                    group_id: Some(group_id),
                })
            }),
            proxy,
        );
    }

    fn par_send_updates<P: IntoParallelIterator<Item = Point3<i32>>>(
        &self,
        points: P,
        proxy: &EventLoopProxy,
    ) {
        Self::send_events(
            points
                .into_par_iter()
                .filter_map(|coords| {
                    Some(ServerEvent::ChunkUpdated {
                        coords,
                        data: ChunkData::new(&self.chunks, &self.light, coords)?.into(),
                        group_id: None,
                    })
                })
                .into_seq_iter(),
            proxy,
        );
    }

    fn generate(&self, coords: Point3<i32>) -> Option<Box<Chunk>> {
        if self.chunks.contains(coords) {
            None
        } else {
            let mut chunk = Box::new(self.generator.generate(coords));
            for (coords, action) in self.actions.actions(coords) {
                chunk.apply_unchecked(coords, action);
            }
            (!chunk.is_empty()).then_some(chunk)
        }
    }

    fn send_unloads<P>(points: P, group_id: Option<GroupId>, proxy: &EventLoopProxy)
    where
        P: IntoIterator<Item = Point3<i32>>,
    {
        Self::send_events(
            points
                .into_iter()
                .map(|coords| ServerEvent::ChunkUnloaded { coords, group_id }),
            proxy,
        );
    }

    fn filter<P>(points: P, area: WorldArea) -> impl Iterator<Item = Point3<i32>>
    where
        P: IntoIterator<Item = Point3<i32>>,
    {
        points
            .into_iter()
            .filter(move |&coords| area.contains(coords))
    }

    fn par_filter<P>(points: P, area: WorldArea) -> impl ParallelIterator<Item = Point3<i32>>
    where
        P: IntoParallelIterator<Item = Point3<i32>>,
    {
        points
            .into_par_iter()
            .filter(move |&coords| area.contains(coords))
    }

    fn send_events<E: IntoIterator<Item = ServerEvent>>(events: E, proxy: &EventLoopProxy) {
        for event in events {
            if proxy.send_event(event).is_err() {
                break;
            }
        }
    }

    fn chunk_area_points<P>(points: P) -> impl Iterator<Item = Point3<i32>>
    where
        P: IntoIterator<Item = Point3<i32>>,
    {
        points
            .into_iter()
            .flat_map(|coords| ChunkArea::chunk_deltas().map(move |delta| coords + delta.cast()))
    }

    fn block_area_points<P>(points: P) -> impl Iterator<Item = Point3<i64>>
    where
        P: IntoIterator<Item = Point3<i64>>,
    {
        points
            .into_iter()
            .flat_map(|coords| BlockArea::deltas().map(move |delta| coords + delta.cast()))
    }
}

impl EventHandler<WorldEvent> for World {
    type Context<'a> = &'a EventLoopProxy;

    fn handle(&mut self, event: &WorldEvent, proxy: Self::Context<'_>) {
        match *event {
            WorldEvent::InitialRenderRequested { area, ray } => {
                let mut loads = area.par_points().collect::<Vec<_>>();
                let inserts = self.par_insert_many(loads.par_iter().copied());

                self.par_light_up(&inserts);

                loads.par_sort_unstable_by_key(|&coords| {
                    utils::magnitude_squared(coords, utils::chunk_coords(ray.origin))
                });

                self.handle(&WorldEvent::BlockHoverRequested { ray }, proxy);

                self.par_send_loads(loads, proxy);
            }
            WorldEvent::WorldAreaChanged { prev, curr, ray } => {
                let inserts = self.par_insert_many(curr.par_exclusive_points(prev));
                let block_updates = self.par_light_up(&inserts);
                let loads = self.exclusive_points(prev, curr).collect();
                let unloads = self.exclusive_points(curr, prev).collect();
                let updates = self.updates(inserts, block_updates, &loads, &unloads);

                self.handle(&WorldEvent::BlockHoverRequested { ray }, proxy);

                Self::send_unloads(unloads, None, proxy);
                self.par_send_loads(loads, proxy);
                self.par_send_updates(Self::par_filter(updates, curr), proxy);
            }
            WorldEvent::BlockHoverRequested { ray } => {
                let hover = ray.cast(SERVER_CONFIG.player.reach.clone()).find(
                    |&BlockIntersection { coords, .. }| {
                        self.chunks
                            .block(coords)
                            .data()
                            .hitbox(coords)
                            .intersects(ray)
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
            WorldEvent::BlockPlaced { block, area, ray } => {
                if let Some(BlockIntersection { coords, normal }) = self.hover {
                    self.apply(
                        coords + normal,
                        normal,
                        BlockAction::Place(block),
                        proxy,
                        area,
                        ray,
                    );
                }
            }
            WorldEvent::BlockDestroyed { area, ray } => {
                if let Some(BlockIntersection { coords, normal }) = self.hover {
                    self.apply(coords, normal, BlockAction::Destroy, proxy, area, ray);
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
            .map_or(Block::AIR, |chunk| chunk[utils::block_coords(coords)])
    }

    fn insert(&mut self, coords: Point3<i32>, chunk: Box<Chunk>) {
        assert!(self.0.insert(coords, chunk).is_none());
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

impl Index<Point3<i32>> for ChunkStore {
    type Output = Chunk;

    fn index(&self, coords: Point3<i32>) -> &Self::Output {
        &self.0[&coords]
    }
}

#[derive(Default)]
struct Branch(FxHashMap<Point3<i32>, FxHashMap<Point3<u8>, BlockAction>>);

type Changes = (
    Vec<Point3<i64>>,
    FxHashSet<Point3<i32>>,
    FxHashSet<Point3<i32>>,
);

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

    fn merge(
        self,
        World {
            chunks,
            heights,
            light,
            actions,
            ..
        }: &mut World,
    ) -> Changes {
        let mut hits = vec![];
        let mut inserts = FxHashSet::default();
        let mut removals = FxHashSet::default();

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
                        removals.insert(chunk_coords);
                    }
                }
                Entry::Vacant(entry) => {
                    let mut actions = actions
                        .into_iter()
                        .filter(|&(_, action)| Block::AIR.is_action_valid(action))
                        .peekable();

                    if actions.peek().is_some() {
                        let chunk = entry.insert(Default::default());
                        for (block_coords, action) in actions {
                            chunk.apply_unchecked(block_coords, action);
                            hits.push((utils::coords((chunk_coords, block_coords)), action));
                        }
                        inserts.insert(chunk_coords);
                    }
                }
            }
        }

        light.extend_placeholders(heights.load_placeholders(&inserts));

        (
            hits.into_iter()
                .inspect(|&(coords, action)| actions.insert(coords, action))
                .flat_map(|(coords, action)| {
                    iter::once(coords).chain(light.apply(chunks, coords, action))
                })
                .collect(),
            inserts,
            removals,
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
    fn new(chunks: &ChunkStore, light: &WorldLight, coords: Point3<i32>) -> Option<Self> {
        chunks.contains(coords).then(|| Self {
            area: chunks.chunk_area(coords),
            area_light: light.chunk_area_light(coords),
        })
    }

    pub fn vertices(&self) -> (Vec<BlockVertex>, Vec<BlockVertex>) {
        let mut vertices = vec![];
        let mut transparent_vertices = vec![];
        let areas = self.block_areas(&mut vertices, &mut transparent_vertices);

        for side in Enum::variants() {
            let mask = SIDE_MASKS[side].map(|c| c.0);
            let delta = SIDE_DELTAS[side];
            let is_negative = delta.sum() == -1;
            let abs_delta = delta.map(i8::abs);

            for axis in 0..=Chunk::DIM as i8 {
                let mut quads = Self::quads(&areas, side, mask, is_negative, abs_delta, axis - 1);
                let mut curr = 0;

                for secondary in 0..Chunk::DIM {
                    let mut main = 0;

                    while main < Chunk::DIM {
                        if let Some(quad) = quads[secondary * Chunk::DIM + main] {
                            let width = Self::width(&quads, curr, main, quad);
                            let height = Self::height(&quads, curr, secondary, quad, width);

                            if let Some(quad) = quad {
                                vertices.extend(quad.vertices(
                                    side,
                                    mask,
                                    [axis as u8, main as u8, secondary as u8],
                                    point![width as u8, height as u8],
                                ));
                            }

                            for secondary in 0..height {
                                for main in 0..width {
                                    quads[curr + secondary * Chunk::DIM + main] = None;
                                }
                            }

                            curr += width;
                            main += width;
                        } else {
                            curr += 1;
                            main += 1;
                        }
                    }
                }
            }
        }

        (vertices, transparent_vertices)
    }

    fn block_areas(
        &self,
        vertices: &mut Vec<BlockVertex>,
        transparent_vertices: &mut Vec<BlockVertex>,
    ) -> DataStore<(BlockArea, BlockAreaLight)> {
        DataStore::from_fn(|coords| {
            let area = self.area.block_area(coords);
            let area_light = self.area_light.block_area_light(coords);
            let data = area.block().data();

            if data.requires_blending {
                transparent_vertices.extend(data.mesh(coords, area, &area_light));
            } else {
                let is_externally_lit = data.is_externally_lit();
                vertices.extend(data.vertices(
                    None,
                    coords,
                    point![1, 1, 1],
                    point![1, 1],
                    area.corner_aos(None, is_externally_lit),
                    area_light.corner_lights(None, area),
                ));
            }

            (area, area_light)
        })
    }

    fn quads(
        areas: &DataStore<(BlockArea, BlockAreaLight)>,
        side: Side,
        mask: Point3<usize>,
        is_negative: bool,
        abs_delta: Vector3<i8>,
        axis: i8,
    ) -> [Option<Option<Quad>>; Chunk::DIM * Chunk::DIM] {
        array::from_fn(|i| {
            let secondary = i / Chunk::DIM;
            let main = i % Chunk::DIM;
            let coords = mask.map(|i| [axis, axis, main as i8, secondary as i8][i]);
            let quad = Self::quad(axis >= 0, areas, side, coords);
            let neighbor = Self::quad(axis < Chunk::DIM as i8 - 1, areas, side, coords + abs_delta);
            if quad == neighbor {
                None
            } else if is_negative {
                neighbor
            } else {
                quad
            }
        })
    }

    fn width(
        quads: &[Option<Option<Quad>>; Chunk::DIM * Chunk::DIM],
        index: usize,
        main: usize,
        quad: Option<Quad>,
    ) -> usize {
        let mut width = 1;
        while main + width < Chunk::DIM && quads[index + width] == Some(quad) {
            width += 1;
        }
        width
    }

    fn height(
        quads: &[Option<Option<Quad>>; Chunk::DIM * Chunk::DIM],
        index: usize,
        secondary: usize,
        quad: Option<Quad>,
        width: usize,
    ) -> usize {
        let mut height = 1;
        'outer: while secondary + height < Chunk::DIM {
            for main in 0..width {
                if quads[index + height * Chunk::DIM + main] != Some(quad) {
                    break 'outer;
                }
            }
            height += 1;
        }
        height
    }

    fn quad(
        cond: bool,
        areas: &DataStore<(BlockArea, BlockAreaLight)>,
        side: Side,
        coords: Point3<i8>,
    ) -> Option<Option<Quad>> {
        cond.then(|| Quad::new(side, &areas[coords.map(|c| c as u8)]))
    }
}

#[derive(Clone, Copy)]
struct Quad {
    block: Block,
    tex_index: u8,
    corner_aos: EnumMap<Corner, u8>,
    corner_lights: EnumMap<Corner, BlockLight>,
}

impl Quad {
    fn new(side: Side, (area, area_light): &(BlockArea, BlockAreaLight)) -> Option<Self> {
        let block = area.block();
        let data = block.data();
        let is_externally_lit = data.is_externally_lit();
        (!data.requires_blending && area.is_side_visible(Some(side))).then(|| Self {
            block,
            tex_index: data.tex_index(),
            corner_aos: area.corner_aos(Some(side), is_externally_lit),
            corner_lights: area_light.corner_lights(Some(side), *area),
        })
    }

    fn vertices(
        self,
        side: Side,
        mask: Point3<usize>,
        [axis, main, secondary]: [u8; 3],
        dims: Point2<u8>,
    ) -> impl Iterator<Item = BlockVertex> {
        self.block.data().vertices(
            Some(side),
            mask.map(|i| [axis, axis, main, secondary][i]),
            mask.map(|i| [0, 0, dims.x, dims.y][i]),
            dims,
            self.corner_aos,
            self.corner_lights,
        )
    }
}

impl PartialEq for Quad {
    fn eq(&self, other: &Self) -> bool {
        self.tex_index == other.tex_index
            && self.corner_aos == other.corner_aos
            && self.corner_lights == other.corner_lights
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
            brightness: Self::brightness(area, area_light),
        }
    }

    fn brightness(area: BlockArea, area_light: &BlockAreaLight) -> BlockLight {
        Enum::variants()
            .flat_map(|side| area_light.corner_lights(side, area).into_values())
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
        area: WorldArea,
        ray: Ray,
    },
    BlockDestroyed {
        area: WorldArea,
        ray: Ray,
    },
}

impl WorldEvent {
    pub fn new(event: &Event, &Player { prev, curr, ray }: &Player) -> Option<Self> {
        match *event {
            Event::Client(ClientEvent::InitialRenderRequested { .. }) => {
                Some(Self::InitialRenderRequested { area: curr, ray })
            }
            Event::Client(ClientEvent::PlayerPositionChanged { .. }) if curr != prev => {
                Some(Self::WorldAreaChanged { prev, curr, ray })
            }
            Event::Client(ClientEvent::PlayerPositionChanged { .. }) => {
                Some(Self::BlockHoverRequested { ray })
            }
            Event::Client(ClientEvent::PlayerOrientationChanged { .. }) => {
                Some(Self::BlockHoverRequested { ray })
            }
            Event::Client(ClientEvent::BlockPlaced { block }) => Some(Self::BlockPlaced {
                block,
                area: curr,
                ray,
            }),
            Event::Client(ClientEvent::BlockDestroyed) => {
                Some(Self::BlockDestroyed { area: curr, ray })
            }
            _ => None,
        }
    }
}
