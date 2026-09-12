pub mod action;
pub mod block;
pub mod chunk;
pub mod height;
pub mod light;

use super::player::{Player, WorldArea};
use crate::{
    client::{ClientEvent, game::world::BlockVertex},
    server::{
        GroupId, SERVER_CONFIG, ServerEvent, ServerSender,
        event_loop::{Event, EventHandler},
    },
    shared::{
        bound::Aabb,
        enum_map::{Enum, EnumMap},
        ray::{BlockIntersection, Intersectable, Ray},
        utils::{self, ParallelIteratorExt},
    },
};
use action::{ActionStore, BlockAction};
use block::{
    Block, BlockLight,
    area::{BlockArea, BlockLightArea},
    data::{Corner, RenderLayer, SIDE_AXES, Side},
};
use chunk::{
    Chunk, ChunkDataStore,
    area::{ChunkArea, ChunkLightArea},
    generator::ChunkGenerator,
    visibility::VisibilityGraph,
};
use crossbeam_channel::SendError;
use height::HeightMap;
use light::WorldLight;
use nalgebra::{Point2, Point3, Vector3, point};
use rayon::{
    iter::{IntoParallelIterator, ParallelIterator},
    slice::ParallelSliceMut,
};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use std::{
    array,
    collections::{VecDeque, hash_map::Entry},
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
            .filter(|coords| !self.chunks.0.contains_key(coords))
            .filter_map(|coords| Some((coords, self.generate(coords)?)))
            .into_seq_iter()
            .map(|(coords, chunk)| {
                self.chunks.0.insert(coords, chunk);
                coords
            })
            .collect()
    }

    #[rustfmt::skip]
    fn apply(
        &mut self,
        coords: Point3<i64>,
        normal: Vector3<i64>,
        action: BlockAction,
        server_tx: &ServerSender,
        area: WorldArea,
        ray: Ray,
    ) {
        let mut branch = Branch::default();
        if !branch.apply(&self.chunks, coords, normal, action) {
            return;
        }

        let Changelog {
            actions,
            mut inserts,
            mut removals,
        } = branch.merge(&mut self.chunks);

        let new_surface_points = self.heights.load_many(inserts.iter().copied());
        self.light.extend_placeholders(&self.heights, new_surface_points);
        let light_updates = self.light.apply(&self.chunks, actions.iter().copied());

        inserts.retain(|&coords| area.client_contains(coords));
        removals.retain(|&coords| area.client_contains(coords));

        let action_updates = actions.iter().map(|&(coords, _)| coords);
        let updates = self.mesh_updates(
            inserts.iter().copied(),
            iter::chain(action_updates, light_updates),
            area,
            &inserts,
            &removals,
        );
        let group_id = GroupId::new(inserts.len() + removals.len() + updates.len());

        self.handle(&WorldEvent::BlockHoverRequested { ray }, server_tx);

        _ = self.send_updates(updates, group_id, server_tx);
        _ = Self::send_unloads(removals, Some(group_id), server_tx);
        _ = self.send_loads(inserts, group_id, server_tx);

        self.actions.extend(actions);
    }

    fn mesh_updates(
        &self,
        inserts: impl IntoIterator<Item = Point3<i32>>,
        block_updates: impl IntoIterator<Item = Point3<i64>>,
        area: WorldArea,
        loads: &FxHashSet<Point3<i32>>,
        unloads: &FxHashSet<Point3<i32>>,
    ) -> FxHashSet<Point3<i32>> {
        let mut updates = inserts
            .into_iter()
            .flat_map(ChunkArea::chunk_points)
            .chain(
                block_updates
                    .into_iter()
                    .flat_map(BlockArea::points)
                    .map(utils::chunk_coords),
            )
            .collect::<FxHashSet<_>>();

        updates.retain(|coords| {
            area.client_contains(*coords)
                && self.chunks.0.contains_key(coords)
                && !loads.contains(coords)
                && !unloads.contains(coords)
        });

        updates
    }

    fn send_loads<P: IntoIterator<Item = Point3<i32>>>(
        &self,
        points: P,
        group_id: GroupId,
        server_tx: &ServerSender,
    ) -> Result<(), SendError<ServerEvent>> {
        points
            .into_iter()
            .map(|coords| ServerEvent::ChunkLoaded {
                coords,
                data: ChunkData::new(&self.chunks, &self.light, coords).into(),
                group_id: Some(group_id),
            })
            .try_for_each(|event| server_tx.send(event))
    }

    fn par_send_loads<P: IntoParallelIterator<Item = Point3<i32>>>(
        &self,
        points: P,
        server_tx: &ServerSender,
    ) -> Result<(), SendError<ServerEvent>> {
        points
            .into_par_iter()
            .map(|coords| ServerEvent::ChunkLoaded {
                coords,
                data: ChunkData::new(&self.chunks, &self.light, coords).into(),
                group_id: None,
            })
            .into_seq_iter()
            .try_for_each(|event| server_tx.send(event))
    }

    fn send_updates<P: IntoIterator<Item = Point3<i32>>>(
        &self,
        points: P,
        group_id: GroupId,
        server_tx: &ServerSender,
    ) -> Result<(), SendError<ServerEvent>> {
        points
            .into_iter()
            .map(|coords| ServerEvent::ChunkUpdated {
                coords,
                data: ChunkData::new(&self.chunks, &self.light, coords).into(),
                group_id: Some(group_id),
            })
            .try_for_each(|event| server_tx.send(event))
    }

    fn par_send_updates<P: IntoParallelIterator<Item = Point3<i32>>>(
        &self,
        points: P,
        server_tx: &ServerSender,
    ) -> Result<(), SendError<ServerEvent>> {
        points
            .into_par_iter()
            .map(|coords| ServerEvent::ChunkUpdated {
                coords,
                data: ChunkData::new(&self.chunks, &self.light, coords).into(),
                group_id: None,
            })
            .into_seq_iter()
            .try_for_each(|event| server_tx.send(event))
    }

    fn generate(&self, coords: Point3<i32>) -> Option<Box<Chunk>> {
        if self.chunks.0.contains_key(&coords) {
            None
        } else {
            let mut chunk = Box::new(self.generator.generate(coords));
            for (coords, action) in self.actions.chunk_actions(coords) {
                chunk.apply_unchecked(coords, action);
            }
            if !chunk.is_empty() {
                chunk.recompute_visibility_graph();
                Some(chunk)
            } else {
                None
            }
        }
    }

    fn send_unloads<P: IntoIterator<Item = Point3<i32>>>(
        points: P,
        group_id: Option<GroupId>,
        server_tx: &ServerSender,
    ) -> Result<(), SendError<ServerEvent>> {
        points
            .into_iter()
            .map(|coords| ServerEvent::ChunkUnloaded { coords, group_id })
            .try_for_each(|event| server_tx.send(event))
    }
}

impl EventHandler<WorldEvent> for World {
    type Context<'a> = &'a ServerSender;

    #[rustfmt::skip]
    fn handle(&mut self, event: &WorldEvent, server_tx: Self::Context<'_>) {
        match *event {
            WorldEvent::PlayerConnected { area, ray } => {
                let inserts = self.par_insert_many(area.par_server_points());

                let new_surface_points = self.heights.load_many(inserts.iter().copied());
                self.light.extend_placeholders(&self.heights, new_surface_points);
                self.light.par_insert_many(&self.chunks, &self.heights, &inserts);

                let mut loads = area
                    .client_points()
                    .filter(|&coords| self.chunks.0.contains_key(&coords))
                    .collect::<Vec<_>>();

                loads.par_sort_unstable_by_key(|&coords| {
                    utils::magnitude_squared(coords, utils::chunk_coords(ray.origin))
                });

                self.handle(&WorldEvent::BlockHoverRequested { ray }, server_tx);

                _ = self.par_send_loads(loads, server_tx);
            }
            WorldEvent::WorldAreaChanged { prev, cur, ray } => {
                let inserts = self.par_insert_many(cur.par_exclusive_server_points(prev));

                let new_surface_points = self.heights.load_many(inserts.iter().copied());
                self.light.extend_placeholders(&self.heights, new_surface_points);
                let light_updates = self.light.par_insert_many(&self.chunks, &self.heights, &inserts);

                let loads = cur
                    .exclusive_client_points(prev)
                    .filter(|&coords| self.chunks.0.contains_key(&coords))
                    .collect();
                let unloads = prev
                    .exclusive_client_points(cur)
                    .filter(|&coords| self.chunks.0.contains_key(&coords))
                    .collect();
                let updates = self.mesh_updates(inserts, light_updates, cur, &loads, &unloads);

                self.handle(&WorldEvent::BlockHoverRequested { ray }, server_tx);

                _ = Self::send_unloads(unloads, None, server_tx);
                _ = self.par_send_loads(loads, server_tx);
                _ = self.par_send_updates(updates, server_tx);
            }
            WorldEvent::BlockHoverRequested { ray } => {
                let hover = ray.cast(SERVER_CONFIG.player.reach).find(
                    |&BlockIntersection { coords, .. }| {
                        self.chunks
                            .block(coords)
                            .data()
                            .model
                            .hitbox(coords)
                            .intersects(ray)
                    },
                );

                if mem::replace(&mut self.hover, hover) != hover {
                    _ = server_tx.send(ServerEvent::BlockHovered(hover.map(
                        |BlockIntersection { coords, .. }| {
                            BlockHoverData::new(
                                coords,
                                &self.chunks.block_area(coords),
                                &self.light.block_light_area(coords),
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
                        server_tx,
                        area,
                        ray,
                    );
                }
            }
            WorldEvent::BlockDestroyed { area, ray } => {
                if let Some(BlockIntersection { coords, normal }) = self.hover {
                    self.apply(coords, normal, BlockAction::Destroy, server_tx, area, ray);
                }
            }
        }
    }
}

#[derive(Default)]
pub struct ChunkStore(FxHashMap<Point3<i32>, Box<Chunk>>);

impl ChunkStore {
    fn get(&self, coords: Point3<i32>) -> Option<&Chunk> {
        self.0.get(&coords).map(|v| &**v)
    }

    fn block(&self, coords: Point3<i64>) -> Block {
        self.get(utils::chunk_coords(coords))
            .map_or_default(|chunk| chunk[utils::block_coords(coords)])
    }

    fn chunk_area(&self, coords: Point3<i32>) -> ChunkArea {
        let mut value = ChunkArea::default();
        for delta in ChunkArea::chunk_deltas() {
            if let Some(chunk) = self.get(coords + delta) {
                let [dx, dy, dz] = delta.into();
                for x in ChunkArea::block_axis_range(dx) {
                    for y in ChunkArea::block_axis_range(dy) {
                        let z = ChunkArea::block_axis_range(dz);
                        value.copy_row(
                            utils::coords(point![dx, dy, dz], point![x, y, z.start])
                                .coords
                                .cast(),
                            chunk.row(point![x, y, z.start], z.len()),
                        );
                    }
                }
            }
        }
        value
    }

    fn block_area(&self, coords: Point3<i64>) -> BlockArea {
        BlockArea::from_fn(|delta| self.block(coords + delta.cast()))
    }
}

impl Index<Point3<i32>> for ChunkStore {
    type Output = Chunk;

    fn index(&self, coords: Point3<i32>) -> &Self::Output {
        &self.0[&coords]
    }
}

#[derive(Default)]
struct Branch {
    actions: ActionStore,
}

struct Changelog {
    actions: Vec<(Point3<i64>, BlockAction)>,
    inserts: FxHashSet<Point3<i32>>,
    removals: FxHashSet<Point3<i32>>,
}

impl Branch {
    fn apply(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        normal: Vector3<i64>,
        action: BlockAction,
    ) -> bool {
        if !self.is_action_valid(chunks, coords, normal, action) {
            false
        } else {
            self.execute_actions(chunks, VecDeque::from([(coords, action)]));
            true
        }
    }

    fn merge(self, chunks: &mut ChunkStore) -> Changelog {
        let mut hits = vec![];
        let mut inserts = FxHashSet::default();
        let mut removals = FxHashSet::default();

        for (chunk_coords, actions) in self.actions.0 {
            match chunks.0.entry(chunk_coords) {
                Entry::Occupied(mut entry) => {
                    let chunk = entry.get_mut();
                    for (block_coords, action) in actions {
                        if chunk.apply(block_coords, action) {
                            hits.push((utils::coords(chunk_coords, block_coords), action));
                        }
                    }
                    if chunk.is_empty() {
                        entry.remove();
                        removals.insert(chunk_coords);
                    } else {
                        chunk.recompute_visibility_graph();
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
                            hits.push((utils::coords(chunk_coords, block_coords), action));
                        }
                        chunk.recompute_visibility_graph();
                        inserts.insert(chunk_coords);
                    }
                }
            }
        }

        Changelog {
            actions: hits,
            inserts,
            removals,
        }
    }

    fn is_action_valid(
        &self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        normal: Vector3<i64>,
        action: BlockAction,
    ) -> bool {
        if !World::Y_RANGE.contains(&utils::chunk_coords(coords).y)
            || !self.block(chunks, coords).is_action_valid(action)
        {
            return false;
        }

        if let BlockAction::Place(block) = action
            && let Some(surface) = block.data().valid_surface
            && (normal != Vector3::y() || self.block(chunks, coords - normal) != surface)
        {
            return false;
        }

        true
    }

    fn execute_actions(
        &mut self,
        chunks: &ChunkStore,
        mut actions: VecDeque<(Point3<i64>, BlockAction)>,
    ) {
        while let Some((coords, action)) = actions.pop_front() {
            if action == BlockAction::Destroy {
                let coords = coords + Vector3::y();
                if self.block(chunks, coords).data().valid_surface.is_some() {
                    actions.push_front((coords, BlockAction::Destroy));
                }
            }
            self.actions.insert(coords, action);
        }
    }

    fn block(&self, chunks: &ChunkStore, coords: Point3<i64>) -> Block {
        let mut block = chunks.block(coords);
        if let Some(action) = self.actions.get(coords) {
            block.apply_unchecked(action);
        }
        block
    }
}

#[derive(Serialize, Deserialize)]
pub struct ChunkData {
    area: ChunkArea,
    light_area: ChunkLightArea,
    pub visibility_graph: VisibilityGraph,
}

impl ChunkData {
    fn new(chunks: &ChunkStore, light: &WorldLight, coords: Point3<i32>) -> Self {
        Self {
            area: chunks.chunk_area(coords),
            light_area: light.chunk_light_area(coords),
            visibility_graph: chunks[coords].visibility_graph,
        }
    }

    pub fn vertices(&self) -> EnumMap<RenderLayer, Vec<BlockVertex>> {
        let mut vertices = EnumMap::<_, Vec<_>>::default();
        let areas = ChunkDataStore::from_fn(|coords| {
            let area = self.area.block_area(coords);
            let light_area = self.light_area.block_light_area(coords);
            let data = area.kernel().data();

            if data.render_layer == RenderLayer::Blended {
                vertices[RenderLayer::Blended].extend(data.mesh(coords, &area, &light_area));
            } else {
                vertices[data.render_layer].extend(data.vertices(
                    None,
                    coords,
                    point![1, 1, 1],
                    point![1, 1],
                    area.corner_aos(None, data.is_externally_lit()),
                    light_area.corner_lights(None, &area),
                ));
            }

            (area, light_area)
        });

        for side in Enum::variants() {
            let axes = SIDE_AXES[side];

            for normal in 0..Chunk::DIM as u8 {
                let mut quads = array::from_fn(|v| {
                    array::from_fn(|u| {
                        let coords = axes.swizzle(point![normal, u as u8, v as u8]);
                        let (area, light_area) = &areas[coords];
                        Quad::new(side, area, light_area)
                    })
                });
                let plane = normal + side.is_positive() as u8;

                for v in 0..Chunk::DIM {
                    let mut u = 0;

                    while u < Chunk::DIM {
                        let Some(quad) = quads[v][u] else {
                            u += 1;
                            continue;
                        };

                        let width = Self::merge_width(&quads, v, u, &quad);
                        let height = Self::merge_height(&quads, v, u, &quad, width);

                        vertices[quad.block.data().render_layer].extend(quad.vertices(
                            side,
                            point![plane, u as u8, v as u8],
                            point![width as u8, height as u8],
                        ));

                        for dv in 0..height {
                            for du in 0..width {
                                quads[v + dv][u + du] = None;
                            }
                        }

                        u += width;
                    }
                }
            }
        }

        vertices
    }

    fn merge_width(
        quads: &[[Option<Quad>; Chunk::DIM]; Chunk::DIM],
        v: usize,
        u: usize,
        quad: &Quad,
    ) -> usize {
        let mut width = 1;
        while u + width < Chunk::DIM && quads[v][u + width].as_ref() == Some(quad) {
            width += 1;
        }
        width
    }

    fn merge_height(
        quads: &[[Option<Quad>; Chunk::DIM]; Chunk::DIM],
        v: usize,
        u: usize,
        quad: &Quad,
        width: usize,
    ) -> usize {
        let mut height = 1;
        'outer: while v + height < Chunk::DIM {
            for du in 0..width {
                if quads[v + height][u + du].as_ref() != Some(quad) {
                    break 'outer;
                }
            }
            height += 1;
        }
        height
    }
}

#[derive(Clone, Copy)]
struct Quad {
    block: Block,
    corner_aos: EnumMap<Corner, u8>,
    corner_lights: EnumMap<Corner, BlockLight>,
}

impl Quad {
    fn new(side: Side, area: &BlockArea, light_area: &BlockLightArea) -> Option<Self> {
        let block = area.kernel();
        let data = block.data();
        let is_externally_lit = data.is_externally_lit();
        (data.render_layer != RenderLayer::Blended && area.is_side_visible(Some(side))).then(|| {
            Self {
                block,
                corner_aos: area.corner_aos(Some(side), is_externally_lit),
                corner_lights: light_area.corner_lights(Some(side), area),
            }
        })
    }

    fn vertices(
        self,
        side: Side,
        coords: Point3<u8>,
        dims: Point2<u8>,
    ) -> impl Iterator<Item = BlockVertex> {
        let axes = SIDE_AXES[side];
        self.block.data().vertices(
            Some(side),
            axes.swizzle(coords),
            axes.swizzle(point![0, dims.x, dims.y]),
            dims,
            self.corner_aos,
            self.corner_lights,
        )
    }
}

impl PartialEq for Quad {
    fn eq(&self, other: &Self) -> bool {
        self.block == other.block
            && self.corner_aos == other.corner_aos
            && self.corner_lights == other.corner_lights
    }
}

impl Eq for Quad {}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct BlockHoverData {
    pub hitbox: Aabb,
    pub brightness: Option<BlockLight>,
}

impl BlockHoverData {
    fn new(coords: Point3<i64>, area: &BlockArea, light_area: &BlockLightArea) -> Self {
        let data = area.kernel().data();
        let hitbox = data.model.hitbox(coords);
        let brightness = data
            .mesh(utils::block_coords(coords), area, light_area)
            .max_by(|a, b| {
                let a = a.world_light(0.0).lum();
                let b = b.world_light(0.0).lum();
                a.total_cmp(&b)
            })
            .map(BlockVertex::light);
        Self { hitbox, brightness }
    }
}

pub enum WorldEvent {
    PlayerConnected {
        area: WorldArea,
        ray: Ray,
    },
    WorldAreaChanged {
        prev: WorldArea,
        cur: WorldArea,
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
    pub fn new(event: &Event, &Player { prev, cur, ray }: &Player) -> Option<Self> {
        match *event {
            Event::Client(ClientEvent::PlayerConnected { .. }) => {
                Some(Self::PlayerConnected { area: cur, ray })
            }
            Event::Client(ClientEvent::PlayerPositionChanged { .. }) if cur != prev => {
                Some(Self::WorldAreaChanged { prev, cur, ray })
            }
            Event::Client(
                ClientEvent::PlayerPositionChanged { .. }
                | ClientEvent::PlayerOrientationChanged { .. },
            ) => Some(Self::BlockHoverRequested { ray }),
            Event::Client(ClientEvent::BlockPlaced(block)) => Some(Self::BlockPlaced {
                block,
                area: cur,
                ray,
            }),
            Event::Client(ClientEvent::BlockDestroyed) => {
                Some(Self::BlockDestroyed { area: cur, ray })
            }
            _ => None,
        }
    }
}
