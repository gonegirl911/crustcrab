pub mod action;
pub mod branch;
pub mod height;
pub mod light;
pub mod mesh;
pub mod scheduler;
pub mod viewer;

use super::{
    block::{
        Block, BlockLight,
        area::{BlockArea, BlockAreaSource, BlockLightArea, BlockLightAreaSource},
    },
    chunk::{Chunk, ChunkReach, area::ChunkArea, generator::ChunkGenerator},
    coords,
    player::{ChunkScope, Player},
};
use crate::{
    client::{PlayerEvent, game::shading::DAY_LIGHT_TABLE},
    server::{
        BatchKind, ChunkEvent, ControlEvent, SERVER_CONFIG,
        connection::{ConnectionEvent, ConnectionId, ConnectionRegistry, RecipientList},
        event_loop::{Event, EventHandler},
    },
    shared::{
        bound::Aabb,
        ray::{BlockIntersection, Intersectable, Ray},
        round_robin::RoundRobin,
    },
};
use action::{ActionStore, BlockAction};
use branch::{Branch, Changelog};
use height::HeightMap;
use light::WorldLight;
use mesh::ChunkData;
use nalgebra::{Point3, Vector3, point};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};
use scheduler::ChunkSchedulerRegistry;
use serde::{Deserialize, Serialize};
use std::{
    iter, mem,
    ops::{Index, Range},
    sync::Arc,
    time::{Duration, Instant},
};
use viewer::ViewerRegistry;

#[derive(Default)]
pub struct World {
    chunks: ChunkStore,
    heights: HeightMap,
    generator: ChunkGenerator,
    actions: ActionStore,
    light: WorldLight,
    viewers: ViewerRegistry,
    schedulers: ChunkSchedulerRegistry,
    tick_rotation: RoundRobin<ConnectionId>,
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
        recipient.send(ChunkEvent::BatchStarted(BatchKind::Delivery));
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

        recipients.send(ChunkEvent::BatchStarted(BatchKind::Broadcast));
        self.send_updates(&recipients, updates);
        Self::send_unloads(&recipients, removals);
        self.send_loads(&recipients, inserts);
        recipients.send(ChunkEvent::BatchEnded);
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
                let deadline = now + TICK_BUDGET;

                let ids = self.schedulers.0.keys().copied().collect::<Vec<_>>();
                let ordered_ids = self.tick_rotation.order(&ids);

                for &id in ordered_ids {
                    let scheduler = self.schedulers.0.get_mut(&id).unwrap();
                    if let Some((from, to)) = scheduler.admit_scope_change(now) {
                        self.sync_scope(id, from, to, connections);
                    }
                    self.deliver_batch(id, connections);

                    self.tick_rotation.advance(id);

                    if Instant::now() > deadline {
                        break;
                    }
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
                            coords::from_parts(point![dx, dy, dz], point![x, y, z.start])
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
        self.get(coords::chunk(coords))
            .map_or_default(|chunk| chunk[coords::block(coords)])
    }
}

impl Index<Point3<i32>> for ChunkStore {
    type Output = Chunk;

    fn index(&self, coords: Point3<i32>) -> &Self::Output {
        &self.0[&coords]
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
            .mesh(coords::block(self.coords), &self.area, &self.light_area)
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

const TICK_BUDGET: Duration = Duration::from_millis(10);
