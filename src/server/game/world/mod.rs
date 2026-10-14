pub mod action;
pub mod block;
pub mod branch;
pub mod chunk;
pub mod coords;
pub mod generator;
pub mod height;
pub mod hover;
pub mod light;
pub mod mesh;
pub mod scheduler;
pub mod viewer;

use super::player::{ChunkScope, Player};
use crate::{
    client::PlayerEvent,
    server::{
        BatchTrigger, ChunkEvent, ControlEvent, SERVER_CONFIG,
        connection::{ConnectionEvent, ConnectionId, ConnectionRegistry, RecipientList},
        event_loop::{Event, EventHandler},
    },
    shared::{
        ray::{BlockIntersection, Intersectable, Ray},
        round_robin::RoundRobin,
    },
};
use action::{BlockAction, BlockActionStore};
use block::Block;
use branch::{Branch, Changelog};
use chunk::{
    Chunk,
    area::{ChunkArea, ChunkReach},
    store::ChunkStore,
};
use generator::ChunkGeneratorPool;
use height::HeightMap;
use hover::BlockHoverData;
use light::WorldLight;
use mesh::ChunkData;
use nalgebra::{Point3, Vector3};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use rustc_hash::FxHashSet;
use scheduler::ChunkSchedulerRegistry;
use std::{
    iter, mem,
    ops::Range,
    sync::Arc,
    time::{Duration, Instant},
};
use viewer::ViewerRegistry;

#[derive(Default)]
pub struct World {
    chunks: ChunkStore,
    heights: HeightMap,
    generators: ChunkGeneratorPool,
    actions: BlockActionStore,
    light: WorldLight,
    viewers: ViewerRegistry,
    schedulers: ChunkSchedulerRegistry,
    tick_rotation: RoundRobin<ConnectionId>,
}

impl World {
    pub const Y_RANGE: Range<i32> = -4..20;

    fn initialize_scope(&mut self, id: ConnectionId, scope: ChunkScope) {
        self.generators.request(
            scope
                .server_points()
                .filter(|&coords| !self.chunks.0.contains_key(&coords)),
            scope.center,
        );

        let scheduler = self.schedulers.0.get_mut(&id).unwrap();
        scheduler.enqueue(
            scope
                .client_points()
                .filter(|&coords| self.chunks.0.contains_key(&coords)),
        );
    }

    #[rustfmt::skip]
    fn apply(
        &mut self,
        coords: Point3<i64>,
        normal: Vector3<i64>,
        action: BlockAction,
        connections: &ConnectionRegistry,
    ) {
        let mut branch = Branch::default();
        if !branch.apply(&self.chunks, coords, normal, action) {
            return;
        }

        let Changelog {
            actions,
            inserts,
            updates: action_updates,
            removals,
        } = branch.merge(&mut self.chunks);

        let new_surface_points = self.heights.load_many(inserts.iter().copied());
        self.light.extend_placeholders(new_surface_points);
        self.light.request_regions(
            &self.chunks,
            action_updates
                .iter()
                .map(|(coords, _)| coords)
                .chain(&inserts)
                .chain(&removals)
                .copied(),
            true,
        );

        let updates = self.mesh_updates(inserts.iter().copied(), action_updates, &inserts, &removals);
        self.broadcast_action(connections, &inserts, &updates, &removals);

        self.actions.extend(actions);
    }

    #[rustfmt::skip]
    fn advance_generators(&mut self, deadline: Instant) -> Vec<Point3<i32>> {
        let demand = self.schedulers.demand();

        self.generators.anticipate(demand, |coords| self.schedulers.server_contains(coords));

        let mut inserts = Vec::with_capacity(demand);
        while inserts.len() < demand {
            let Some((coords, chunk)) = self.generators.poll_deadline(deadline) else {
                break;
            };

            if self.schedulers.server_contains(coords)
                && let Some(chunk) = self.actions.apply(coords, chunk)
            {
                self.chunks.0.insert(coords, chunk.into());
                inserts.push(coords);
            }

            if Instant::now() > deadline {
                break;
            }
        }
        inserts
    }

    #[rustfmt::skip]
    fn integrate_inserts(&mut self, inserts: &[Point3<i32>]) {
        let new_surface_points = self.heights.load_many(inserts.iter().copied());
        self.light.extend_placeholders(new_surface_points);
        self.light.request_regions(&self.chunks, inserts.iter().copied(), false);

        let loads = inserts.iter().copied().collect();
        let updates = self.mesh_updates(inserts.iter().copied(), [], &loads, &Default::default());

        for scheduler in self.schedulers.0.values_mut() {
            let scope = scheduler.admitted;
            scheduler.enqueue(
                iter::chain(&loads, &updates)
                    .copied()
                    .filter(|&coords| scope.client_contains(coords)),
            );
        }
    }

    fn enqueue_relights(&mut self, relights: &FxHashSet<Point3<i32>>) {
        for scheduler in self.schedulers.0.values_mut() {
            let scope = scheduler.admitted;
            scheduler.enqueue(
                relights
                    .iter()
                    .copied()
                    .filter(|&coords| scope.client_contains(coords)),
            );
        }
    }

    fn sync_scope(
        &mut self,
        id: ConnectionId,
        from: ChunkScope,
        to: ChunkScope,
        connections: &ConnectionRegistry,
    ) {
        self.generators.request(
            to.exclusive_server_points(&from)
                .filter(|&coords| !self.chunks.0.contains_key(&coords)),
            to.center,
        );

        let scheduler = self.schedulers.0.get_mut(&id).unwrap();
        scheduler.enqueue(
            to.exclusive_client_points(&from)
                .filter(|&coords| self.chunks.0.contains_key(&coords)),
        );

        Self::send_unloads(
            &connections.one(id),
            from.exclusive_client_points(&to)
                .filter(|&coords| self.chunks.0.contains_key(&coords)),
        );
    }

    fn deliver_scope(&mut self, id: ConnectionId, connections: &ConnectionRegistry) {
        let scheduler = self.schedulers.0.get_mut(&id).unwrap();
        let is_settled = |coords| {
            ChunkArea::chunk_deltas().all(|delta| {
                let coords = coords + delta.cast();
                !self.generators.is_pending(coords) && !self.light.is_pending(coords)
            })
        };
        let Some((points, allowance)) = scheduler.admit_batch(is_settled) else {
            return;
        };

        let recipient = connections.one(id);
        recipient.send(ChunkEvent::BatchStarted(BatchTrigger::Scheduler {
            allowance,
        }));
        self.par_send_loads(&recipient, points);
        recipient.send(ChunkEvent::BatchEnded);
    }

    #[rustfmt::skip]
    fn broadcast_action(
        &mut self,
        connections: &ConnectionRegistry,
        inserts: &FxHashSet<Point3<i32>>,
        updates: &FxHashSet<Point3<i32>>,
        removals: &FxHashSet<Point3<i32>>,
    ) {
        let batch = iter::chain(inserts, updates)
            .chain(removals)
            .copied()
            .collect::<Vec<_>>();
        let recipient_ids = self.schedulers.client_containing(&batch).collect::<Vec<_>>();
        let recipients = connections.many(recipient_ids.iter().copied());

        recipients.send(ChunkEvent::BatchStarted(BatchTrigger::Action));
        Self::send_unloads(&recipients, removals.iter().copied());
        self.send_loads(&recipients, iter::chain(inserts, updates).copied());
        recipients.send(ChunkEvent::BatchEnded);
    }

    fn mesh_updates(
        &self,
        inserts: impl IntoIterator<Item = Point3<i32>>,
        updates: impl IntoIterator<Item = (Point3<i32>, ChunkReach)>,
        loads: &FxHashSet<Point3<i32>>,
        unloads: &FxHashSet<Point3<i32>>,
    ) -> FxHashSet<Point3<i32>> {
        inserts
            .into_iter()
            .flat_map(|coords| ChunkArea::chunk_deltas().map(move |delta| coords + delta.cast()))
            .chain(updates.into_iter().flat_map(|(coords, reach)| {
                reach.into_iter().map(move |delta| coords + delta.cast())
            }))
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
                        BlockHoverData::new(&self.chunks, &self.light, coords).into()
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
                        connections,
                    );
                    self.handle(&WorldEvent::BlockHoverRequested { id, aim }, connections);
                }
            }
            WorldEvent::BlockDestroyed { id, aim } => {
                let viewer = &self.viewers.0[&id];
                if let Some(BlockIntersection { coords, normal }) = viewer.hover {
                    self.apply(coords, normal, BlockAction::Destroy, connections);
                    self.handle(&WorldEvent::BlockHoverRequested { id, aim }, connections);
                }
            }
            #[rustfmt::skip]
            WorldEvent::Tick(now) => {
                let deadline = Instant::now() + GENERATOR_BUDGET;
                let inserts = self.advance_generators(deadline);
                self.integrate_inserts(&inserts);

                let deadline = Instant::now() + LIGHT_BUDGET;
                let light_updates = self.light.settle(&self.chunks, deadline);
                let relights = self.mesh_updates(
                    [],
                    light_updates,
                    &Default::default(),
                    &Default::default(),
                );
                self.enqueue_relights(&relights);

                let ids = self.schedulers.0.keys().copied().collect::<Vec<_>>();
                let ordered_ids = self.tick_rotation.order(&ids);
                let deadline = Instant::now() + SCOPE_BUDGET;

                for &id in ordered_ids {
                    let scheduler = self.schedulers.0.get_mut(&id).unwrap();
                    if let Some((from, to)) = scheduler.admit_scope_change(now) {
                        self.sync_scope(id, from, to, connections);
                    }
                    self.deliver_scope(id, connections);

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
        chunks_per_second: Option<f32>,
    },
    Tick(Instant),
}

impl WorldEvent {
    pub fn new(event: &Event, player: Option<&Player>) -> Option<Self> {
        let (id, player_event, &Player { prev, cur, aim }) = match event {
            Event::Connection(event) => {
                return Some(Self::Connection(event.clone()));
            }
            Event::Player(id, event) => (*id, event, player?),
            &Event::Tick(now) => {
                return Some(WorldEvent::Tick(now));
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

const LIGHT_BUDGET: Duration = Duration::from_millis(7);
const GENERATOR_BUDGET: Duration = Duration::from_millis(10);
const SCOPE_BUDGET: Duration = Duration::from_millis(10);
