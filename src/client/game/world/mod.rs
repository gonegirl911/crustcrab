pub mod mesh;

use super::player::frustum::{Cullable, Frustum};
use crate::{
    client::{
        CLIENT_CONFIG, PlayerEvent,
        event_loop::{Event, EventHandler},
        renderer::{
            Renderer,
            effect::PostProcessor,
            render_pipeline::RenderPipeline,
            texture::screen::DepthBuffer,
            utils::{Immediates, TotalOrd, Vertex, read_wgsl},
        },
    },
    enum_map,
    server::{
        BatchTrigger, ChunkEvent,
        game::{
            block::data::{RenderLayer, SIDE_DELTAS},
            chunk::{
                Chunk,
                visibility::{SideSet, VisibilityGraph},
            },
            coords,
            player::ChunkScope,
            world::mesh::ChunkData,
        },
    },
    shared::{
        enum_map::EnumMap, indexmap::FxIndexMap, net::compression::Compressed, pool::JobPool, utils,
    },
};
use crossbeam_channel::Sender;
use mesh::{BlockImmediates, BlockVertex, ChunkMesh};
use nalgebra::Point3;
use rustc_hash::FxHashMap;
use std::{
    cmp::Reverse,
    collections::{VecDeque, hash_map::Entry},
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

pub struct World {
    meshes: FxHashMap<Point3<i32>, ChunkMesh>,
    render_pipelines: EnumMap<RenderLayer, RenderPipeline>,
    revisions: FxHashMap<Point3<i32>, RevisionTracker>,
    open_batch_id: BatchId,
    pending_batches: FxHashMap<BatchId, BatchTracker>,
    pool: JobPool<ChunkInput, ChunkOutput>,
}

impl World {
    pub fn new(
        renderer: &Renderer,
        player_uniform_bind_group_layout: &wgpu::BindGroupLayout,
        shading_uniform_bind_group_layout: &wgpu::BindGroupLayout,
        shading_texture_bind_group_layout: &wgpu::BindGroupLayout,
        textures_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let bind_group_layouts = &[
            player_uniform_bind_group_layout,
            shading_uniform_bind_group_layout,
            shading_texture_bind_group_layout,
            textures_bind_group_layout,
        ];
        let render_pipelines = enum_map! {
            RenderLayer::Opaque => {
                Self::render_pipeline(renderer, bind_group_layouts, Some("fs_main"), None)
            }
            RenderLayer::Cutout => {
                Self::render_pipeline(renderer, bind_group_layouts, Some("fs_cutout"), None)
            }
            RenderLayer::Blended => Self::render_pipeline(
                renderer,
                bind_group_layouts,
                Some("fs_cutout"),
                Some(wgpu::BlendState::ALPHA_BLENDING),
            ),
        };
        let pool = JobPool::new(THREAD_HEADROOM, Self::compute);
        Self {
            meshes: Default::default(),
            render_pipelines,
            revisions: Default::default(),
            open_batch_id: BatchId::NIL,
            pending_batches: Default::default(),
            pool,
        }
    }

    #[expect(clippy::too_many_arguments)]
    pub fn draw_opaque(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_uniform_bind_group: &wgpu::BindGroup,
        shading_uniform_bind_group: &wgpu::BindGroup,
        shading_texture_bind_group: &wgpu::BindGroup,
        textures_bind_group: &wgpu::BindGroup,
        depth_view: &wgpu::TextureView,
        anchor: Point3<f64>,
        frustum: &Frustum,
    ) -> Vec<Point3<i32>> {
        let visible_points = self.cull_chunks(frustum);
        let mut cutout_parts = vec![];
        let mut blended_points = vec![];
        let bind_groups = [
            player_uniform_bind_group,
            shading_uniform_bind_group,
            shading_texture_bind_group,
            textures_bind_group,
        ];
        let mut render_pass = Self::render_pass(view, encoder, depth_view, true);

        self.render_pipelines[RenderLayer::Opaque].bind(&mut render_pass, bind_groups);

        for coords in visible_points {
            let Some(mesh) = self.meshes.get(&coords) else {
                continue;
            };

            if let Some(opaque_part) = &mesh.opaque_part {
                BlockImmediates::new(coords, anchor).set(&mut render_pass);
                opaque_part.draw(&mut render_pass);
            }

            if let Some(cutout_part) = &mesh.cutout_part {
                cutout_parts.push((coords, cutout_part));
            }

            if mesh.blended_part.is_some() {
                blended_points.push(coords);
            }
        }

        self.render_pipelines[RenderLayer::Cutout].bind(&mut render_pass, bind_groups);

        for (coords, cutout_part) in cutout_parts {
            BlockImmediates::new(coords, anchor).set(&mut render_pass);
            cutout_part.draw(&mut render_pass);
        }

        blended_points
    }

    #[expect(clippy::too_many_arguments)]
    pub fn draw_blended(
        &mut self,
        renderer: &Renderer,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        mut blended_points: Vec<Point3<i32>>,
        player_uniform_bind_group: &wgpu::BindGroup,
        shading_uniform_bind_group: &wgpu::BindGroup,
        shading_texture_bind_group: &wgpu::BindGroup,
        textures_bind_group: &wgpu::BindGroup,
        depth_view: &wgpu::TextureView,
        origin: Point3<f64>,
        anchor: Point3<f64>,
    ) {
        blended_points.sort_unstable_by_key(|&coords| {
            Reverse(utils::distance_squared(coords, coords::chunk(origin)))
        });

        let mut render_pass = Self::render_pass(view, encoder, depth_view, false);

        self.render_pipelines[RenderLayer::Blended].bind(
            &mut render_pass,
            [
                player_uniform_bind_group,
                shading_uniform_bind_group,
                shading_texture_bind_group,
                textures_bind_group,
            ],
        );

        for coords in blended_points {
            let mesh = self.meshes.get_mut(&coords).unwrap();
            let blended_part = mesh.blended_part.as_mut().unwrap();
            let displacement = coords.cast() * Chunk::DIM as f64 - origin;
            BlockImmediates::new(coords, anchor).set(&mut render_pass);
            blended_part.draw(renderer, &mut render_pass, displacement, |&coords| {
                TotalOrd((coords.coords.cast() + displacement).magnitude_squared())
            });
        }
    }

    fn bump_revision(&mut self, coords: Point3<i32>) -> Revision {
        let revision = self.revisions.entry(coords).or_default();
        revision.current.0 += 1;
        revision.pending += 1;
        revision.current
    }

    fn join_open_batch(&mut self) -> Option<&BatchApplication> {
        if self.open_batch_id == BatchId::NIL {
            return None;
        }

        let batch = self.pending_batches.get_mut(&self.open_batch_id).unwrap();
        batch.expected += 1;
        Some(&batch.application)
    }

    fn integrate_remesh(
        &mut self,
        renderer: &Renderer,
        remesh: ChunkRemesh,
        batch_id: BatchId,
        player_tx: &Sender<PlayerEvent>,
    ) {
        let Some(batch) = self.pending_batches.get_mut(&batch_id) else {
            self.apply_remesh(renderer, remesh);
            return;
        };

        if let BatchFeedback::Report { meshed, .. } = &mut batch.feedback {
            *meshed += remesh.data.is_some() as usize;
        }

        match &mut batch.application {
            BatchApplication::Immediate { received } => {
                *received += 1;
                self.apply_remesh(renderer, remesh);
            }
            BatchApplication::Atomic { remeshes } => {
                remeshes.push(remesh);
            }
        }

        if batch_id != self.open_batch_id {
            self.flush_batch_if_completed(renderer, batch_id, player_tx);
        }
    }

    fn flush_batch_if_completed(
        &mut self,
        renderer: &Renderer,
        batch_id: BatchId,
        player_tx: &Sender<PlayerEvent>,
    ) {
        let Entry::Occupied(entry) = self.pending_batches.entry(batch_id) else {
            return;
        };
        let batch = entry.get();

        let received = batch.application.received();
        if received < batch.expected {
            return;
        }

        let BatchTracker {
            application,
            feedback,
            ..
        } = entry.remove();

        if let BatchApplication::Atomic { remeshes } = application {
            for remesh in remeshes {
                self.apply_remesh(renderer, remesh);
            }
        }

        let BatchFeedback::Report {
            opened_at,
            allowance,
            meshed,
        } = feedback
        else {
            return;
        };

        let is_measured = meshed > 0 && received >= allowance;
        let elapsed = opened_at.elapsed();
        let rate = meshed as f32 / elapsed.as_secs_f32();
        _ = player_tx.send(PlayerEvent::ChunkBatchAcknowledged {
            chunks_per_second: is_measured.then_some(rate),
        });
    }

    fn cull_chunks(&self, frustum: &Frustum) -> impl Iterator<Item = Point3<i32>> {
        let origin = coords::chunk(frustum.origin);
        let mut queue = VecDeque::from([origin]);
        let mut visited = FxIndexMap::from_iter([(origin, SideSet::default())]);
        let scope = ChunkScope {
            center: origin,
            radius: CLIENT_CONFIG.player.render_distance as i32,
        };

        while let Some(coords) = queue.pop_front() {
            let displacement = coords - origin;
            let visibility_graph = self.meshes.get(&coords).map(|mesh| mesh.visibility_graph);
            let sources = visited[&coords];

            for (exit, delta) in *SIDE_DELTAS {
                if delta.cast().dot(&displacement) < 0 {
                    continue;
                }

                let neighbor_coords = coords + delta.cast();

                if !scope.client_contains(neighbor_coords) {
                    continue;
                }

                if !Chunk::bounding_sphere(neighbor_coords).is_visible(frustum) {
                    continue;
                }

                if coords != origin
                    && let Some(graph) = visibility_graph
                    && !sources.into_iter().any(|source| {
                        if !graph.connected(source, exit) {
                            return false;
                        }

                        if exit != source.opp() {
                            return true;
                        }

                        displacement.amax() <= displacement[exit.axis()].abs() + 1
                    })
                {
                    continue;
                }

                let neighbor_source = exit.opp();
                let neighbor_sources = visited.entry(neighbor_coords).or_default();
                if !neighbor_sources.contains(neighbor_source) {
                    neighbor_sources.insert(neighbor_source);
                    queue.push_back(neighbor_coords);
                }
            }
        }

        visited.into_keys()
    }

    fn apply_remesh(
        &mut self,
        renderer: &Renderer,
        ChunkRemesh {
            coords,
            data,
            snapshot_revision,
        }: ChunkRemesh,
    ) {
        let Entry::Occupied(mut revision_entry) = self.revisions.entry(coords) else {
            unreachable!();
        };
        let revision = revision_entry.get_mut();
        let is_stale = snapshot_revision < revision.current;

        revision.pending -= 1;
        if revision.pending == 0 {
            revision_entry.remove();
        }

        if is_stale {
            return;
        }

        if let Some(ChunkRemeshData {
            vertices,
            visibility_graph,
        }) = data
            && let Some(mesh) = ChunkMesh::new(renderer, vertices.each_deref(), visibility_graph)
        {
            self.meshes.insert(coords, mesh);
        } else {
            self.meshes.remove(&coords);
        }
    }

    fn render_pipeline(
        renderer: &Renderer,
        bind_group_layouts: &[&wgpu::BindGroupLayout],
        fragment_entry_point: Option<&str>,
        blend: Option<wgpu::BlendState>,
    ) -> RenderPipeline {
        RenderPipeline::builder()
            .renderer(renderer)
            .shader_desc(read_wgsl("assets/shaders/block.wgsl"))
            .bind_group_layouts(bind_group_layouts)
            .immediate_size(BlockImmediates::SIZE)
            .buffers(&[BlockVertex::desc()])
            .cull_mode(wgpu::Face::Back)
            .depth_stencil(wgpu::DepthStencilState {
                format: DepthBuffer::FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            })
            .maybe_fragment_entry_point(fragment_entry_point)
            .format(PostProcessor::FORMAT)
            .maybe_blend(blend)
            .build()
    }

    fn compute(
        ChunkInput {
            data,
            snapshot_revision,
            batch_id,
        }: ChunkInput,
    ) -> ChunkOutput {
        ChunkOutput {
            coords: data.coords,
            vertices: data.vertices(),
            visibility_graph: data.visibility_graph,
            snapshot_revision,
            batch_id,
        }
    }

    fn render_pass<'a>(
        view: &wgpu::TextureView,
        encoder: &'a mut wgpu::CommandEncoder,
        depth_view: &wgpu::TextureView,
        is_initial: bool,
    ) -> wgpu::RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(Default::default()),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: if is_initial {
                        wgpu::LoadOp::Clear(1.0)
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        })
    }
}

impl EventHandler for World {
    type Context<'a> = (&'a Sender<PlayerEvent>, &'a Renderer);

    fn handle(&mut self, event: &Event, (player_tx, renderer): Self::Context<'_>) {
        match event {
            Event::ChunkEvent(event) => match event {
                ChunkEvent::Loaded(Compressed { inner: data, .. }) => {
                    let snapshot_revision = self.bump_revision(data.coords);
                    let application = self.join_open_batch();
                    let has_priority = matches!(application, Some(BatchApplication::Atomic { .. }));
                    self.pool.submit(
                        ChunkInput {
                            data: data.clone(),
                            snapshot_revision,
                            batch_id: self.open_batch_id,
                        },
                        has_priority,
                    );
                }
                &ChunkEvent::Unloaded(coords) => {
                    let snapshot_revision = self.bump_revision(coords);
                    self.join_open_batch();
                    self.integrate_remesh(
                        renderer,
                        ChunkRemesh {
                            coords,
                            data: None,
                            snapshot_revision,
                        },
                        self.open_batch_id,
                        player_tx,
                    );
                }
                &ChunkEvent::BatchStarted(trigger) => {
                    self.flush_batch_if_completed(renderer, self.open_batch_id, player_tx);
                    self.open_batch_id = BatchId::new(trigger);
                    self.pending_batches.insert(
                        self.open_batch_id,
                        BatchTracker {
                            application: BatchApplication::from(trigger),
                            expected: 0,
                            feedback: BatchFeedback::new(trigger, Instant::now()),
                        },
                    );
                }
                ChunkEvent::BatchEnded => {
                    self.flush_batch_if_completed(renderer, self.open_batch_id, player_tx);
                    self.open_batch_id = BatchId::NIL;
                }
            },
            Event::AboutToWait => {
                let drain_budget = Duration::from_millis(CLIENT_CONFIG.app.drain_budget_ms);
                let deadline = Instant::now() + drain_budget;

                while let Ok(ChunkOutput {
                    coords,
                    vertices,
                    visibility_graph,
                    batch_id,
                    snapshot_revision,
                }) = self.pool.try_recv()
                {
                    self.integrate_remesh(
                        renderer,
                        ChunkRemesh {
                            coords,
                            data: Some(ChunkRemeshData {
                                vertices,
                                visibility_graph,
                            }),
                            snapshot_revision,
                        },
                        batch_id,
                        player_tx,
                    );

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
struct RevisionTracker {
    current: Revision,
    pending: u32,
}

#[derive(Clone, Copy, PartialEq, PartialOrd, Default)]
struct Revision(u32);

struct BatchTracker {
    application: BatchApplication,
    expected: usize,
    feedback: BatchFeedback,
}

enum BatchApplication {
    Immediate { received: usize },
    Atomic { remeshes: Vec<ChunkRemesh> },
}

impl BatchApplication {
    fn received(&self) -> usize {
        match self {
            &Self::Immediate { received } => received,
            Self::Atomic { remeshes } => remeshes.len(),
        }
    }
}

impl From<BatchTrigger> for BatchApplication {
    fn from(trigger: BatchTrigger) -> Self {
        match trigger {
            BatchTrigger::Scheduler { .. } => Self::Immediate { received: 0 },
            BatchTrigger::Action => Self::Atomic {
                remeshes: Default::default(),
            },
        }
    }
}

enum BatchFeedback {
    Report {
        opened_at: Instant,
        allowance: usize,
        meshed: usize,
    },
    Skip,
}

impl BatchFeedback {
    fn new(trigger: BatchTrigger, now: Instant) -> Self {
        match trigger {
            BatchTrigger::Scheduler { allowance } => Self::Report {
                opened_at: now,
                allowance,
                meshed: 0,
            },
            BatchTrigger::Action => Self::Skip,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct BatchId {
    id: Uuid,
    trigger: BatchTrigger,
}

impl BatchId {
    const NIL: Self = Self {
        id: Uuid::nil(),
        trigger: BatchTrigger::Action,
    };

    fn new(trigger: BatchTrigger) -> Self {
        Self {
            id: Uuid::new_v4(),
            trigger,
        }
    }
}

struct ChunkRemesh {
    coords: Point3<i32>,
    data: Option<ChunkRemeshData>,
    snapshot_revision: Revision,
}

struct ChunkRemeshData {
    vertices: EnumMap<RenderLayer, Vec<BlockVertex>>,
    visibility_graph: VisibilityGraph,
}

struct ChunkInput {
    data: Arc<ChunkData>,
    snapshot_revision: Revision,
    batch_id: BatchId,
}

struct ChunkOutput {
    coords: Point3<i32>,
    vertices: EnumMap<RenderLayer, Vec<BlockVertex>>,
    visibility_graph: VisibilityGraph,
    snapshot_revision: Revision,
    batch_id: BatchId,
}

const THREAD_HEADROOM: usize = 1;
