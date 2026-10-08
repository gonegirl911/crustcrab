pub mod action;
pub mod height;
pub mod light;
pub mod scheduler;

use super::player::{ChunkScope, Player};
use crate::{
    client::{
        PlayerEvent,
        game::{shading::DAY_LIGHT_TABLE, world::BlockVertex},
    },
    server::{
        ChunkEvent, ControlEvent, SERVER_CONFIG,
        connection::{ConnectionEvent, ConnectionId, ConnectionRegistry, RecipientList},
        event_loop::{Event, EventHandler},
        game::{
            block::{
                Block, BlockLight,
                area::{
                    BlockArea, BlockAreaSource, BlockContext, BlockLightArea,
                    BlockLightAreaSource,
                },
                data::{Corner, RenderLayer, SIDE_AXES, Side},
            },
            chunk::{
                Chunk, ChunkReach,
                area::{ChunkArea, ChunkLightArea},
                generator::ChunkGenerator,
                visibility::VisibilityGraph,
            },
            coords,
        },
    },
    shared::{
        bound::Aabb,
        enum_map::{Enum, EnumMap},
        ray::{BlockIntersection, Intersectable, Ray},
    },
};
use action::{ActionStore, BlockAction};
use height::HeightMap;
use light::WorldLight;
use nalgebra::{Point2, Point3, Vector3, point};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};
use scheduler::ChunkSchedulerRegistry;
use serde::{Deserialize, Serialize};
use std::{
    array,
    collections::{VecDeque, hash_map::Entry},
    iter, mem,
    ops::{Index, Range},
    sync::Arc,
    time::Instant,
};

#[derive(Default)]
pub struct World {
    chunks: ChunkStore,
    heights: HeightMap,
    generator: ChunkGenerator,
    actions: ActionStore,
    light: WorldLight,
    viewers: ViewerRegistry,
    schedulers: ChunkSchedulerRegistry,
}

impl World {
    pub const Y_RANGE: Range<i32> = -4..20;

    #[rustfmt::skip]
    fn initialize_scope(&mut self, id: ConnectionId, scope: ChunkScope) {
        let inserts = self.par_insert_many(scope.par_server_points());

        let new_surface_points = self.heights.load_many(inserts.iter().copied());
        self.light.extend_placeholders(new_surface_points);
        self.light.par_insert_many(&self.chunks, &self.heights, &inserts, false);

        let loads = scope
            .client_points()
            .filter(|&coords| self.chunks.0.contains_key(&coords))
            .collect::<Vec<_>>();

        let scheduler = self.schedulers.0.get_mut(&id).unwrap();
        scheduler.queue(loads, scope.center);
    }

    #[rustfmt::skip]
    fn sync_scope(
        &mut self,
        id: ConnectionId,
        from: ChunkScope,
        to: ChunkScope,
        connections: &ConnectionRegistry,
    ) {
        let inserts = self.par_insert_many(to.par_exclusive_server_points(&from));

        let new_surface_points = self.heights.load_many(inserts.iter().copied());
        self.light.extend_placeholders(new_surface_points);
        let light_updates = self.light.par_insert_many(&self.chunks, &self.heights, &inserts, true);

        let loads = to
            .exclusive_client_points(&from)
            .filter(|&coords| self.chunks.0.contains_key(&coords))
            .collect();
        let unloads = from
            .exclusive_client_points(&to)
            .filter(|&coords| self.chunks.0.contains_key(&coords))
            .collect();
        let mut updates = self.mesh_updates(inserts, light_updates, &loads, &unloads);

        updates.retain(|&coords| to.client_contains(coords));

        let scheduler = self.schedulers.0.get_mut(&id).unwrap();
        scheduler.queue(loads, to.center);

        let recipient = connections.one(id);
        Self::send_unloads(&recipient, unloads);
        self.par_send_updates(&recipient, updates);
    }

    fn deliver_batch(&mut self, id: ConnectionId, connections: &ConnectionRegistry) {
        let scheduler = self.schedulers.0.get_mut(&id).unwrap();
        let Some(points) = scheduler.admit_batch() else {
            return;
        };

        let recipient = connections.one(id);
        recipient.send(ChunkEvent::BatchStarted);
        self.par_send_loads(&recipient, points);
        recipient.send(ChunkEvent::BatchEnded);
    }

    fn apply(
        &mut self,
        coords: Point3<i64>,
        normal: Vector3<i64>,
        action: BlockAction,
        id: ConnectionId,
        connections: &ConnectionRegistry,
        aim: Ray,
    ) {
        let mut branch = Branch::default();
        if !branch.apply(&self.chunks, coords, normal, action) {
            return;
        }

        let Changelog {
            actions,
            inserts,
            removals,
            updates: action_updates,
        } = branch.merge(&mut self.chunks);

        let new_surface_points = self.heights.load_many(inserts.iter().copied());
        self.light.extend_placeholders(new_surface_points);
        let light_updates = self.light.apply(&self.chunks, actions.iter().copied());

        let updates = self.mesh_updates(
            inserts.iter().copied(),
            iter::chain(action_updates, light_updates),
            &inserts,
            &removals,
        );

        self.handle(&WorldEvent::BlockHoverRequested { id, aim }, connections);

        self.broadcast_batch(connections, updates, removals, inserts);

        self.actions.extend(actions);
    }

    fn par_insert_many<P>(&mut self, points: P) -> Vec<Point3<i32>>
    where
        P: IntoParallelIterator<Item = Point3<i32>>,
    {
        points
            .into_par_iter()
            .filter(|coords| !self.chunks.0.contains_key(coords))
            .filter_map(|coords| Some((coords, self.generate(coords)?)))
            .collect_vec_list()
            .into_iter()
            .flatten()
            .map(|(coords, chunk)| {
                self.chunks.0.insert(coords, chunk);
                coords
            })
            .collect()
    }

    #[rustfmt::skip]
    fn broadcast_batch(
        &mut self,
        connections: &ConnectionRegistry,
        updates: FxHashSet<Point3<i32>>,
        removals: FxHashSet<Point3<i32>>,
        inserts: FxHashSet<Point3<i32>>,
    ) {
        let batch = iter::chain(&inserts, &removals)
            .chain(&updates)
            .copied()
            .collect::<Vec<_>>();
        let recipient_ids = self.schedulers.client_containing(&batch).collect::<Vec<_>>();
        let recipients = connections.many(recipient_ids.iter().copied());

        recipients.send(ChunkEvent::BatchStarted);
        self.send_updates(&recipients, updates);
        Self::send_unloads(&recipients, removals);
        self.send_loads(&recipients, inserts);
        recipients.send(ChunkEvent::BatchEnded);

        for id in recipient_ids {
            let scheduler = self.schedulers.0.get_mut(&id).unwrap();
            scheduler.unacknowledged_batches += 1;
        }
    }

    fn mesh_updates(
        &self,
        inserts: impl IntoIterator<Item = Point3<i32>>,
        updates: impl IntoIterator<Item = (Point3<i32>, ChunkReach)>,
        loads: &FxHashSet<Point3<i32>>,
        unloads: &FxHashSet<Point3<i32>>,
    ) -> FxHashSet<Point3<i32>> {
        updates
            .into_iter()
            .flat_map(|(coords, reach)| reach.into_iter().map(move |delta| coords + delta))
            .chain(
                inserts
                    .into_iter()
                    .flat_map(|coords| ChunkArea::chunk_deltas().map(move |delta| coords + delta)),
            )
            .filter(|coords| {
                self.chunks.0.contains_key(coords)
                    && !loads.contains(coords)
                    && !unloads.contains(coords)
            })
            .collect()
    }

    fn send_loads<P>(&self, recipients: &RecipientList, points: P)
    where
        P: IntoIterator<Item = Point3<i32>>,
    {
        points
            .into_iter()
            .map(|coords| {
                ChunkEvent::Loaded(
                    Arc::new(ChunkData::new(&self.chunks, &self.light, coords)).into(),
                )
            })
            .for_each(|event| recipients.send(event));
    }

    fn par_send_loads<P>(&self, recipients: &RecipientList, points: P)
    where
        P: IntoParallelIterator<Item = Point3<i32>>,
    {
        points
            .into_par_iter()
            .map(|coords| {
                ChunkEvent::Loaded(
                    Arc::new(ChunkData::new(&self.chunks, &self.light, coords)).into(),
                )
            })
            .collect_vec_list()
            .into_iter()
            .flatten()
            .for_each(|event| recipients.send(event));
    }

    fn send_updates<P>(&self, recipients: &RecipientList, points: P)
    where
        P: IntoIterator<Item = Point3<i32>>,
    {
        points
            .into_iter()
            .map(|coords| {
                ChunkEvent::Updated(
                    Arc::new(ChunkData::new(&self.chunks, &self.light, coords)).into(),
                )
            })
            .for_each(|event| recipients.send(event));
    }

    fn par_send_updates<P>(&self, recipients: &RecipientList, points: P)
    where
        P: IntoParallelIterator<Item = Point3<i32>>,
    {
        points
            .into_par_iter()
            .map(|coords| {
                ChunkEvent::Updated(
                    Arc::new(ChunkData::new(&self.chunks, &self.light, coords)).into(),
                )
            })
            .collect_vec_list()
            .into_iter()
            .flatten()
            .for_each(|event| recipients.send(event));
    }

    fn generate(&self, coords: Point3<i32>) -> Option<Box<Chunk>> {
        if self.chunks.0.contains_key(&coords) {
            return None;
        }

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

    fn send_unloads<P: IntoIterator<Item = Point3<i32>>>(recipients: &RecipientList, points: P) {
        points
            .into_iter()
            .map(ChunkEvent::Unloaded)
            .for_each(|event| recipients.send(event));
    }
}

impl EventHandler<WorldEvent> for World {
    type Context<'a> = &'a ConnectionRegistry;

    fn handle(&mut self, event: &WorldEvent, connections: Self::Context<'_>) {
        self.viewers.handle(event, ());
        self.schedulers.handle(event, ());

        match *event {
            WorldEvent::JoinRequested { id, scope, aim } => {
                self.initialize_scope(id, scope);
                self.handle(&WorldEvent::BlockHoverRequested { id, aim }, connections);
            }
            WorldEvent::ChunkScopeChanged { id, aim, .. } => {
                self.handle(&WorldEvent::BlockHoverRequested { id, aim }, connections);
            }
            WorldEvent::BlockHoverRequested { id, aim } => {
                let viewer = self.viewers.0.get_mut(&id).unwrap();

                let hover = aim.cast(SERVER_CONFIG.player.reach).find(
                    |&BlockIntersection { coords, .. }| {
                        self.chunks
                            .block(coords)
                            .data()
                            .hitbox(coords)
                            .intersects(aim)
                    },
                );

                if mem::replace(&mut viewer.hover, hover) != hover {
                    let data = hover.map(|BlockIntersection { coords, .. }| {
                        BlockHoverData {
                            coords,
                            area: self.chunks.block_area(coords),
                            light_area: self.light.block_light_area(coords),
                        }
                        .into()
                    });
                    connections.one(id).send(ControlEvent::BlockHovered(data));
                }
            }
            WorldEvent::BlockPlaced { id, block, aim } => {
                let viewer = &self.viewers.0[&id];
                if let Some(BlockIntersection { coords, normal }) = viewer.hover {
                    self.apply(
                        coords + normal,
                        normal,
                        BlockAction::Place(block),
                        id,
                        connections,
                        aim,
                    );
                }
            }
            WorldEvent::BlockDestroyed { id, aim } => {
                let viewer = &self.viewers.0[&id];
                if let Some(BlockIntersection { coords, normal }) = viewer.hover {
                    self.apply(coords, normal, BlockAction::Destroy, id, connections, aim);
                }
            }
            WorldEvent::Tick => {
                let now = Instant::now();

                for id in self.schedulers.0.keys().copied().collect::<Vec<_>>() {
                    let scheduler = self.schedulers.0.get_mut(&id).unwrap();

                    if let Some((from, to)) = scheduler.admit_scope_change(now) {
                        self.sync_scope(id, from, to, connections);
                    }

                    self.deliver_batch(id, connections);
                }
            }
            _ => {}
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
                let [dx, dy, dz] = delta.into();
                for x in ChunkArea::axis_range(dx) {
                    for y in ChunkArea::axis_range(dy) {
                        let z = ChunkArea::axis_range(dz);
                        value.copy_row(
                            coords::coords(point![dx, dy, dz], point![x, y, z.start])
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

    fn get(&self, coords: Point3<i32>) -> Option<&Chunk> {
        self.0.get(&coords).map(|v| &**v)
    }

    fn block(&self, coords: Point3<i64>) -> Block {
        self.get(coords::chunk_coords(coords))
            .map_or_default(|chunk| chunk[coords::block_coords(coords)])
    }
}

impl Index<Point3<i32>> for ChunkStore {
    type Output = Chunk;

    fn index(&self, coords: Point3<i32>) -> &Self::Output {
        &self.0[&coords]
    }
}

#[derive(Default)]
struct ViewerRegistry(FxHashMap<ConnectionId, Viewer>);

impl EventHandler<WorldEvent> for ViewerRegistry {
    type Context<'a> = ();

    fn handle(&mut self, event: &WorldEvent, (): Self::Context<'_>) {
        match *event {
            WorldEvent::JoinRequested { id, .. } => {
                self.0.insert(id, Viewer { hover: None });
            }
            WorldEvent::Connection(ConnectionEvent::Closed(id)) => {
                self.0.remove(&id);
            }
            _ => {}
        }
    }
}

struct Viewer {
    hover: Option<BlockIntersection>,
}

#[derive(Default)]
struct Branch {
    actions: ActionStore,
}

struct Changelog {
    actions: Vec<(Point3<i64>, BlockAction)>,
    inserts: FxHashSet<Point3<i32>>,
    removals: FxHashSet<Point3<i32>>,
    updates: Vec<(Point3<i32>, ChunkReach)>,
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
        let mut updates = vec![];

        for (chunk_coords, actions) in self.actions.0 {
            match chunks.0.entry(chunk_coords) {
                Entry::Occupied(mut entry) => {
                    let chunk = entry.get_mut();
                    let mut reach = ChunkReach::default();

                    for (block_coords, action) in actions {
                        if chunk.apply(block_coords, action) {
                            hits.push((coords::coords(chunk_coords, block_coords), action));
                            reach.insert_block(block_coords);
                        }
                    }

                    if chunk.is_empty() {
                        entry.remove();
                        removals.insert(chunk_coords);
                    } else {
                        chunk.recompute_visibility_graph();
                    }

                    if !reach.is_empty() {
                        updates.push((chunk_coords, reach));
                    }
                }
                Entry::Vacant(entry) => {
                    let mut actions = actions
                        .into_iter()
                        .filter(|&(_, action)| Block::AIR.is_action_valid(action))
                        .peekable();

                    if actions.peek().is_none() {
                        continue;
                    }

                    let chunk = entry.insert(Default::default());
                    let mut reach = ChunkReach::default();

                    for (block_coords, action) in actions {
                        chunk.apply_unchecked(block_coords, action);
                        hits.push((coords::coords(chunk_coords, block_coords), action));
                        reach.insert_block(block_coords);
                    }

                    chunk.recompute_visibility_graph();
                    inserts.insert(chunk_coords);
                    updates.push((chunk_coords, reach));
                }
            }
        }

        Changelog {
            actions: hits,
            inserts,
            removals,
            updates,
        }
    }

    fn is_action_valid(
        &self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        normal: Vector3<i64>,
        action: BlockAction,
    ) -> bool {
        if !World::Y_RANGE.contains(&coords::chunk_coords(coords).y)
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
    pub coords: Point3<i32>,
    area: ChunkArea,
    light_area: ChunkLightArea,
    pub visibility_graph: VisibilityGraph,
}

impl ChunkData {
    fn new(chunks: &ChunkStore, light: &WorldLight, coords: Point3<i32>) -> Self {
        Self {
            coords,
            area: chunks.chunk_area(coords),
            light_area: light.chunk_light_area(coords),
            visibility_graph: chunks[coords].visibility_graph,
        }
    }

    pub fn vertices(&self) -> EnumMap<RenderLayer, Vec<BlockVertex>> {
        let mut vertices = EnumMap::<_, Vec<_>>::default();

        for coords in Chunk::points() {
            let area = self.area.block_area_view(coords);
            let light_area = self.light_area.block_light_area_view(coords);
            let data = area.kernel().data();
            vertices[data.render_layer].extend(data.vertices(
                None,
                coords,
                point![1, 1, 1],
                point![1, 1],
                area.corner_aos(None, data.is_externally_lit()),
                light_area.corner_lights(None, &area),
            ));
        }

        for side in Enum::variants() {
            let axes = SIDE_AXES[side];

            for normal in 0..Chunk::DIM as u8 {
                let mut quads = array::from_fn(|v| {
                    array::from_fn(|u| {
                        let coords = axes.swizzle(point![normal, u as u8, v as u8]);
                        Quad::new(
                            side,
                            &self.area.block_area_view(coords),
                            &self.light_area.block_light_area_view(coords),
                        )
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
                            quads[v + dv][u..u + width].fill(None);
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
    fn new(
        side: Side,
        area: &BlockContext<impl BlockAreaSource>,
        light_area: &BlockContext<impl BlockLightAreaSource>,
    ) -> Option<Self> {
        let block = area.kernel();
        let data = block.data();
        let is_externally_lit = data.is_externally_lit();
        area.is_side_visible(Some(side)).then(|| Self {
            block,
            corner_aos: area.corner_aos(Some(side), is_externally_lit),
            corner_lights: light_area.corner_lights(Some(side), area),
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

#[derive(Serialize, Deserialize)]
pub struct BlockHoverData {
    coords: Point3<i64>,
    area: BlockArea,
    light_area: BlockLightArea,
}

impl BlockHoverData {
    pub fn hitbox(&self) -> Aabb {
        self.area.kernel().data().hitbox(self.coords)
    }

    pub fn brightness(&self) -> BlockLight {
        self.area
            .kernel()
            .data()
            .mesh(
                coords::block_coords(self.coords),
                &self.area,
                &self.light_area,
            )
            .max_by(|a, b| {
                let a = DAY_LIGHT_TABLE.value(a.light(), 0).lum();
                let b = DAY_LIGHT_TABLE.value(b.light(), 0).lum();
                a.total_cmp(&b)
            })
            .map_or(self.light_area.kernel(), |v| v.light())
    }
}

pub enum WorldEvent {
    Connection(ConnectionEvent),
    JoinRequested {
        id: ConnectionId,
        scope: ChunkScope,
        aim: Ray,
    },
    ChunkScopeChanged {
        id: ConnectionId,
        scope: ChunkScope,
        aim: Ray,
    },
    BlockHoverRequested {
        id: ConnectionId,
        aim: Ray,
    },
    BlockPlaced {
        id: ConnectionId,
        block: Block,
        aim: Ray,
    },
    BlockDestroyed {
        id: ConnectionId,
        aim: Ray,
    },
    ChunkBatchAcknowledged {
        id: ConnectionId,
        chunks_per_second: f32,
    },
    Tick,
}

impl WorldEvent {
    pub fn new(event: &Event, player: Option<&Player>) -> Option<Self> {
        let (id, player_event, &Player { prev, cur, aim }) = match event {
            Event::Connection(event) => {
                return Some(Self::Connection(event.clone()));
            }
            Event::Player(id, event) => (*id, event, player?),
            Event::Tick(_) => {
                return Some(WorldEvent::Tick);
            }
        };
        match *player_event {
            PlayerEvent::JoinRequested { .. } => Some(Self::JoinRequested {
                id,
                scope: cur,
                aim,
            }),
            PlayerEvent::Position { .. } if cur != prev => Some(Self::ChunkScopeChanged {
                id,
                scope: cur,
                aim,
            }),
            PlayerEvent::Position { .. } | PlayerEvent::Orientation { .. } => {
                Some(Self::BlockHoverRequested { id, aim })
            }
            PlayerEvent::BlockPlaced(block) => Some(Self::BlockPlaced { id, block, aim }),
            PlayerEvent::BlockDestroyed => Some(Self::BlockDestroyed { id, aim }),
            PlayerEvent::ChunkBatchAcknowledged { chunks_per_second } => {
                Some(Self::ChunkBatchAcknowledged {
                    id,
                    chunks_per_second,
                })
            }
            _ => None,
        }
    }
}
