use super::player::frustum::{Cullable, Frustum};
use crate::{
    client::{
        CLIENT_CONFIG,
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
        GroupId, ServerEvent,
        game::{
            player::WorldArea,
            world::{
                ChunkData,
                block::{
                    BlockLight,
                    data::{RenderLayer, SIDE_DELTAS, SideShade},
                },
                chunk::{
                    Chunk,
                    visibility::{SideSet, VisibilityGraph},
                },
            },
        },
    },
    shared::{color::Rgb, enum_map::EnumMap, indexmap::FxIndexMap, pool::ThreadPool, utils},
};
use bitfield::{BitRange, BitRangeMut};
use bytemuck::{Pod, Zeroable};
use nalgebra::{Point2, Point3, point};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    cmp::Reverse,
    collections::{VecDeque, hash_map::Entry},
    iter, mem,
    sync::Arc,
    time::Instant,
};
use uuid::Uuid;
use winit::event::WindowEvent;

pub struct World {
    meshes: FxHashMap<Point3<i32>, (ChunkMesh, Instant)>,
    render_pipelines: EnumMap<RenderLayer, RenderPipeline>,
    unloaded: FxHashSet<Point3<i32>>,
    groups: FxHashMap<Uuid, Vec<Result<ChunkOutput, Point3<i32>>>>,
    group_workers: ThreadPool<(ChunkInput, GroupId), (ChunkOutput, GroupId)>,
    workers: ThreadPool<ChunkInput, ChunkOutput>,
}

impl World {
    pub fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
        shading_bind_group_layout: &wgpu::BindGroupLayout,
        textures_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let bind_group_layouts = &[
            player_bind_group_layout,
            sky_bind_group_layout,
            shading_bind_group_layout,
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
        let group_workers = ThreadPool::new(|(input, group_id)| (Self::compute(input), group_id));
        let workers = ThreadPool::new(Self::compute);
        Self {
            meshes: Default::default(),
            render_pipelines,
            unloaded: Default::default(),
            groups: Default::default(),
            group_workers,
            workers,
        }
    }

    #[expect(clippy::too_many_arguments)]
    pub fn draw<F: FnOnce(&mut wgpu::CommandEncoder)>(
        &mut self,
        renderer: &Renderer,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_bind_group: &wgpu::BindGroup,
        sky_bind_group: &wgpu::BindGroup,
        shading_bind_group: &wgpu::BindGroup,
        textures_bind_group: &wgpu::BindGroup,
        depth_view: &wgpu::TextureView,
        frustum: &Frustum,
        intermediate_action: F,
    ) {
        let visible_points = self.cull_chunks(frustum);
        let mut cutout_parts = vec![];
        let mut blended_points = vec![];
        let bind_groups = [
            player_bind_group,
            sky_bind_group,
            shading_bind_group,
            textures_bind_group,
        ];

        {
            let mut render_pass = Self::render_pass(view, encoder, depth_view, true);

            self.render_pipelines[RenderLayer::Opaque].bind(&mut render_pass, bind_groups);

            for coords in visible_points {
                let Some((mesh, _)) = self.meshes.get(&coords) else {
                    continue;
                };

                if let Some(opaque_part) = &mesh.opaque_part {
                    BlockImmediates::new(coords).set(&mut render_pass);
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
                BlockImmediates::new(coords).set(&mut render_pass);
                cutout_part.draw(&mut render_pass);
            }
        }

        intermediate_action(encoder);

        let mut render_pass = Self::render_pass(view, encoder, depth_view, false);

        self.render_pipelines[RenderLayer::Blended].bind(&mut render_pass, bind_groups);

        blended_points.sort_unstable_by_key(|&coords| {
            Reverse(utils::magnitude_squared(
                coords,
                utils::chunk_coords(frustum.origin),
            ))
        });

        for coords in blended_points {
            let (mesh, _) = self.meshes.get_mut(&coords).unwrap();
            let blended_part = mesh.blended_part.as_mut().unwrap();
            let delta = coords.cast() * Chunk::DIM as f32 - frustum.origin;
            BlockImmediates::new(coords).set(&mut render_pass);
            blended_part.draw(renderer, &mut render_pass, |&coords| {
                TotalOrd((coords.coords + delta).magnitude_squared())
            });
        }
    }

    fn send(&self, input: ChunkInput, group_id: Option<GroupId>) {
        if let Some(group_id) = group_id {
            self.group_workers.send((input, group_id)).unwrap();
        } else {
            self.workers.send(input).unwrap();
        }
    }

    fn process_output(
        &mut self,
        renderer: &Renderer,
        output: Result<ChunkOutput, Point3<i32>>,
        group_id: Option<GroupId>,
    ) {
        let Some(GroupId {
            id: group_id,
            size: group_size,
        }) = group_id
        else {
            self.apply_output(renderer, output);
            return;
        };

        match self.groups.entry(group_id) {
            Entry::Occupied(mut entry) => {
                let group = entry.get_mut();
                if group.len() == group_size - 1 {
                    for output in iter::chain(entry.remove(), [output]) {
                        self.apply_output(renderer, output);
                    }
                } else {
                    group.push(output);
                }
            }
            Entry::Vacant(entry) => {
                if group_size == 1 {
                    self.apply_output(renderer, output);
                } else {
                    let mut group = Vec::with_capacity(group_size);
                    group.push(output);
                    entry.insert(group);
                }
            }
        }
    }

    fn apply_output(&mut self, renderer: &Renderer, output: Result<ChunkOutput, Point3<i32>>) {
        let ChunkOutput {
            coords,
            vertices,
            visibility_graph,
            updated_at,
        } = match output {
            Ok(output) => output,
            Err(coords) => {
                self.meshes.remove(&coords);
                return;
            }
        };

        if self.unloaded.contains(&coords) {
            return;
        }

        match self.meshes.entry(coords) {
            Entry::Occupied(mut entry) => {
                let (chunk_mesh, last_updated_at) = entry.get_mut();
                if *last_updated_at < updated_at {
                    if let Some(mesh) =
                        ChunkMesh::new(renderer, vertices.each_deref(), visibility_graph)
                    {
                        *chunk_mesh = mesh;
                    } else {
                        entry.remove();
                    }
                }
            }
            Entry::Vacant(entry) => {
                if let Some(mesh) =
                    ChunkMesh::new(renderer, vertices.each_deref(), visibility_graph)
                {
                    entry.insert((mesh, updated_at));
                }
            }
        }
    }

    #[rustfmt::skip]
    fn cull_chunks(&self, frustum: &Frustum) -> impl Iterator<Item = Point3<i32>> {
        let origin = utils::chunk_coords(frustum.origin);
        let mut queue = VecDeque::from([origin]);
        let mut visited = FxIndexMap::from_iter([(origin, SideSet::default())]);
        let area = WorldArea {
            center: origin,
            radius: CLIENT_CONFIG.player.render_distance as i32,
        };

        while let Some(coords) = queue.pop_front() {
            let offset = coords - origin;
            let visibility_graph = self.meshes.get(&coords).map(|(mesh, _)| mesh.visibility_graph);
            let sources = visited[&coords];

            for (exit, delta) in *SIDE_DELTAS {
                if delta.cast().dot(&offset) < 0 {
                    continue;
                }

                let neighbor_coords = coords + delta.cast();

                if !area.client_contains(neighbor_coords) {
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

                        offset.amax() <= offset[exit.axis()].abs() + 1
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
            coords,
            data,
            updated_at,
        }: ChunkInput,
    ) -> ChunkOutput {
        ChunkOutput {
            coords,
            vertices: data.vertices(),
            visibility_graph: data.visibility_graph,
            updated_at,
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
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        match event {
            Event::ServerEvent(event) => match event {
                ServerEvent::ChunkLoaded {
                    coords,
                    data,
                    group_id,
                } => {
                    self.unloaded.remove(coords);
                    self.send(
                        ChunkInput {
                            coords: *coords,
                            data: data.clone(),
                            updated_at: Instant::now(),
                        },
                        *group_id,
                    );
                }
                &ServerEvent::ChunkUnloaded { coords, group_id } => {
                    self.unloaded.insert(coords);
                    self.process_output(renderer, Err(coords), group_id);
                }
                ServerEvent::ChunkUpdated {
                    coords,
                    data,
                    group_id,
                } => {
                    self.send(
                        ChunkInput {
                            coords: *coords,
                            data: data.clone(),
                            updated_at: Instant::now(),
                        },
                        *group_id,
                    );
                }
                _ => {}
            },
            Event::WindowEvent(WindowEvent::RedrawRequested) => {
                while let Ok((output, group_id)) = self.group_workers.try_recv() {
                    self.process_output(renderer, Ok(output), Some(group_id));
                }

                while let Ok(output) = self.workers.try_recv() {
                    self.process_output(renderer, Ok(output), None);
                }
            }
            _ => {}
        }
    }
}

struct ChunkInput {
    coords: Point3<i32>,
    data: Arc<ChunkData>,
    updated_at: Instant,
}

struct ChunkOutput {
    coords: Point3<i32>,
    vertices: EnumMap<RenderLayer, Vec<BlockVertex>>,
    visibility_graph: VisibilityGraph,
    updated_at: Instant,
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
        let is_empty = opaque_part.is_none()
            && cutout_part.is_none()
            && blended_part.is_none()
            && visibility_graph == VisibilityGraph::ALL_CONNECTED;
        (!is_empty).then_some(Self {
            opaque_part,
            cutout_part,
            blended_part,
            visibility_graph,
        })
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
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

    fn coords(self) -> Point3<u8> {
        point![
            self.data[0].bit_range(4, 0),
            self.data[0].bit_range(9, 5),
            self.data[0].bit_range(14, 10),
        ]
    }

    fn side_shade(self) -> SideShade {
        unsafe { mem::transmute::<u8, _>(self.data[0].bit_range(24, 23)) }
    }

    fn ao(self) -> u8 {
        self.data[0].bit_range(26, 25)
    }

    pub fn light(self) -> BlockLight {
        BlockLight(self.data[1])
    }

    fn skylight(self) -> Rgb<u8> {
        self.light().skylight()
    }

    fn torchlight(self) -> Rgb<u8> {
        self.light().torchlight()
    }

    pub fn light_factor(self, nightness: f32) -> Rgb<f32> {
        let shading = &CLIENT_CONFIG.shading;

        let side_factors = shading.side_factors;
        let ao_factor_min = shading.ao_factor_min;
        let ao_factor_max = shading.ao_factor_max;
        let ao_max = 3.0;

        let side_shade = self.side_shade();
        let ao = self.ao() as f32;
        let side_factor = side_factors[side_shade];
        let ao_factor = utils::lerp(ao_factor_min, ao_factor_max, ao / ao_max);
        self.world_light(nightness) * (1.0 - ao_factor) * side_factor
    }

    pub fn world_light(self, nightness: f32) -> Rgb<f32> {
        let sky = &CLIENT_CONFIG.sky;
        let shading = &CLIENT_CONFIG.shading;

        let sunlight_intensity = sky.sunlight_intensity(nightness);
        let light_attenuation = shading.light_attenuation;
        let light_max = BlockLight::COMPONENT_MAX;

        let skylight = self.skylight();
        let torchlight = self.torchlight();
        let global_light = skylight.map(|c| light_attenuation.powi((light_max - c) as i32));
        let local_light = torchlight.map(|c| light_attenuation.powi((light_max - c) as i32));
        (global_light * sunlight_intensity + local_light).saturate()
    }
}

impl Vertex for BlockVertex {
    const ATTRIBS: &[wgpu::VertexAttribute] = &wgpu::vertex_attr_array![0 => Uint32x2];
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct BlockImmediates {
    chunk_coords: Point3<f32>,
}

impl BlockImmediates {
    fn new(chunk_coords: Point3<i32>) -> Self {
        Self {
            chunk_coords: chunk_coords.cast(),
        }
    }
}

impl Immediates for BlockImmediates {}
