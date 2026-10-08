use super::player::frustum::{Cullable, Frustum};
use crate::{
    client::{
        CLIENT_CONFIG, PlayerEvent,
        event_loop::{Event, EventHandler},
        renderer::{
            Renderer,
            buffer::{MemoryState, VertexBuffer},
            effect::PostProcessor,
            render_pipeline::RenderPipeline,
            texture::screen::DepthBuffer,
            utils::{BlendedMesh, Immediates, TotalOrd, Vertex, read_wgsl},
        },
    },
    enum_map,
    server::{
        ChunkEvent,
        game::{
            block::{
                BlockLight,
                data::{RenderLayer, SIDE_DELTAS, SideShade},
            },
            chunk::{
                Chunk,
                visibility::{SideSet, VisibilityGraph},
            },
            coords,
            player::ChunkScope,
            world::ChunkData,
        },
    },
    shared::{
        ema::Ema, enum_map::EnumMap, indexmap::FxIndexMap, net::compression::Compressed,
        pool::JobPool, utils,
    },
};
use bitfield::{BitRange, BitRangeMut};
use bytemuck::{Pod, Zeroable};
use crossbeam_channel::Sender;
use nalgebra::{Point2, Point3, point};
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
    pending_batches: FxHashMap<BatchId, ChunkBatch>,
    chunk_rate: Ema,
    workers: JobPool<ChunkInput, ChunkOutput>,
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
        let workers = JobPool::new(Self::compute);
        Self {
            meshes: Default::default(),
            render_pipelines,
            revisions: Default::default(),
            open_batch_id: BatchId::NIL,
            pending_batches: Default::default(),
            chunk_rate: Ema::new(CHUNK_RATE_SAMPLE_WEIGHT),
            workers,
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
            Reverse(utils::distance_squared(coords, coords::chunk_coords(origin)))
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

    fn join_open_batch(&mut self) -> bool {
        if self.open_batch_id == BatchId::NIL {
            return false;
        }

        self.pending_batches
            .entry(self.open_batch_id)
            .and_modify(|batch| batch.expected += 1)
            .or_insert_with(|| ChunkBatch {
                changes: Default::default(),
                expected: 1,
                started_at: Instant::now(),
            });
        true
    }

    fn batch_or_apply_change(
        &mut self,
        renderer: &Renderer,
        change: ChunkChange,
        batch_id: BatchId,
        player_tx: &Sender<PlayerEvent>,
    ) {
        if let Some(batch) = self.pending_batches.get_mut(&batch_id) {
            batch.changes.push(change);
            if batch_id != self.open_batch_id {
                self.flush_batch_if_completed(renderer, batch_id, player_tx);
            }
        } else {
            self.apply_change(renderer, change);
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

        if batch.changes.len() < batch.expected {
            return;
        }

        let ChunkBatch {
            changes,
            expected,
            started_at,
        } = entry.remove();

        for change in changes {
            self.apply_change(renderer, change);
        }

        let elapsed = started_at.elapsed();
        let sample = expected as f32 / elapsed.as_secs_f32();
        let chunks_per_second = self.chunk_rate.smooth(sample);
        _ = player_tx.send(PlayerEvent::ChunkBatchAcknowledged { chunks_per_second });
    }

    fn cull_chunks(&self, frustum: &Frustum) -> impl Iterator<Item = Point3<i32>> {
        let origin = coords::chunk_coords(frustum.origin);
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

    fn apply_change(
        &mut self,
        renderer: &Renderer,
        ChunkChange {
            coords,
            data,
            snapshot_revision,
        }: ChunkChange,
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

        match data {
            ChunkChangeData::Remesh {
                vertices,
                visibility_graph,
            } => {
                self.apply_remesh(renderer, coords, vertices, visibility_graph);
            }
            ChunkChangeData::Unload => {
                self.meshes.remove(&coords);
            }
        }
    }

    fn apply_remesh(
        &mut self,
        renderer: &Renderer,
        coords: Point3<i32>,
        vertices: EnumMap<RenderLayer, Vec<BlockVertex>>,
        visibility_graph: VisibilityGraph,
    ) {
        match ChunkMesh::new(renderer, vertices.each_deref(), visibility_graph) {
            Some(mesh) => {
                self.meshes.insert(coords, mesh);
            }
            None => {
                self.meshes.remove(&coords);
            }
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
                    let has_priority = self.join_open_batch();
                    self.workers.submit(
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
                    self.batch_or_apply_change(
                        renderer,
                        ChunkChange {
                            coords,
                            data: ChunkChangeData::Unload,
                            snapshot_revision,
                        },
                        self.open_batch_id,
                        player_tx,
                    );
                }
                ChunkEvent::Updated(Compressed { inner: data, .. }) => {
                    let snapshot_revision = self.bump_revision(data.coords);
                    let has_priority = self.join_open_batch();
                    self.workers.submit(
                        ChunkInput {
                            data: data.clone(),
                            batch_id: self.open_batch_id,
                            snapshot_revision,
                        },
                        has_priority,
                    );
                }
                ChunkEvent::BatchStarted => {
                    self.flush_batch_if_completed(renderer, self.open_batch_id, player_tx);
                    self.open_batch_id = BatchId::new();
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
                }) = self.workers.try_recv()
                {
                    self.batch_or_apply_change(
                        renderer,
                        ChunkChange {
                            coords,
                            data: ChunkChangeData::Remesh {
                                vertices,
                                visibility_graph,
                            },
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

struct ChunkBatch {
    changes: Vec<ChunkChange>,
    expected: usize,
    started_at: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct BatchId(Uuid);

impl BatchId {
    const NIL: Self = Self(Uuid::nil());

    fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

struct ChunkChange {
    coords: Point3<i32>,
    data: ChunkChangeData,
    snapshot_revision: Revision,
}

enum ChunkChangeData {
    Remesh {
        vertices: EnumMap<RenderLayer, Vec<BlockVertex>>,
        visibility_graph: VisibilityGraph,
    },
    Unload,
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

struct ChunkMesh {
    opaque_part: Option<VertexBuffer<BlockVertex>>,
    cutout_part: Option<VertexBuffer<BlockVertex>>,
    blended_part: Option<BlendedMesh<Point3<f32>, BlockVertex>>,
    visibility_graph: VisibilityGraph,
}

impl ChunkMesh {
    fn new(
        renderer: &Renderer,
        vertices: EnumMap<RenderLayer, &[BlockVertex]>,
        visibility_graph: VisibilityGraph,
    ) -> Option<Self> {
        let opaque_part = VertexBuffer::try_new(
            renderer,
            MemoryState::Immutable(vertices[RenderLayer::Opaque]),
        );
        let cutout_part = VertexBuffer::try_new(
            renderer,
            MemoryState::Immutable(vertices[RenderLayer::Cutout]),
        );
        let blended_part = BlendedMesh::try_new(renderer, vertices[RenderLayer::Blended], |v| {
            v.iter()
                .fold(Point3::default(), |acc, v| acc + v.coords().coords)
                .cast()
                / v.len() as f32
        });
        if opaque_part.is_some()
            || cutout_part.is_some()
            || blended_part.is_some()
            || visibility_graph != VisibilityGraph::ALL_CONNECTED
        {
            Some(Self {
                opaque_part,
                cutout_part,
                blended_part,
                visibility_graph,
            })
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct BlockVertex {
    data: [u32; 2],
}

impl BlockVertex {
    pub fn new(
        coords: Point3<u8>,
        tex_index: u8,
        tex_coords: Point2<u8>,
        side_shade: SideShade,
        ao: u8,
        light: BlockLight,
    ) -> Self {
        let mut data = [0; 2];
        data[0].set_bit_range(4, 0, coords.x);
        data[0].set_bit_range(9, 5, coords.y);
        data[0].set_bit_range(14, 10, coords.z);
        data[0].set_bit_range(22, 15, tex_index);
        data[0].set_bit_range(31, 27, tex_coords.x);
        data[1].set_bit_range(31, 27, tex_coords.y);
        data[0].set_bit_range(24, 23, side_shade as u8);
        data[0].set_bit_range(26, 25, ao);
        data[1].set_bit_range(26, 0, light.0);
        Self { data }
    }

    fn coords(&self) -> Point3<u8> {
        point![
            self.data[0].bit_range(4, 0),
            self.data[0].bit_range(9, 5),
            self.data[0].bit_range(14, 10),
        ]
    }

    pub fn light(&self) -> BlockLight {
        BlockLight(self.data[1])
    }
}

impl Vertex for BlockVertex {
    const ATTRIBS: &[wgpu::VertexAttribute] = &wgpu::vertex_attr_array![0 => Uint32x2];
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct BlockImmediates {
    chunk_coords: Point3<f32>,
}

impl BlockImmediates {
    fn new(chunk_coords: Point3<i32>, anchor: Point3<f64>) -> Self {
        let anchor = coords::chunk_coords(anchor);
        let chunk_coords = chunk_coords - anchor.coords;
        Self {
            chunk_coords: chunk_coords.cast(),
        }
    }
}

impl Immediates for BlockImmediates {}

const CHUNK_RATE_SAMPLE_WEIGHT: f32 = 0.25;
