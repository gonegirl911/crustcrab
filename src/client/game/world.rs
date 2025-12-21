use super::player::frustum::{Cullable, Frustum};
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{
            Renderer,
            buffer::{MemoryState, Vertex, VertexBuffer},
            effect::PostProcessor,
            program::{Immediates, Program},
            shader::read_wgsl,
            texture::screen::DepthBuffer,
            utils::{TotalOrd, TransparentMesh},
        },
    },
    server::{
        GroupId, ServerEvent,
        game::world::{
            ChunkData,
            block::{BlockLight, data::Face},
            chunk::Chunk,
        },
    },
    shared::{pool::ThreadPool, utils},
};
use bitfield::{BitRange, BitRangeMut};
use bytemuck::{Pod, Zeroable};
use nalgebra::{Point2, Point3, point};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{cmp::Reverse, collections::hash_map::Entry, iter, sync::Arc, time::Instant};
use uuid::Uuid;
use winit::event::WindowEvent;

pub struct World {
    meshes: FxHashMap<Point3<i32>, (ChunkMesh, Instant)>,
    program: Program,
    unloaded: FxHashSet<Point3<i32>>,
    groups: FxHashMap<Uuid, Vec<Result<ChunkOutput, Point3<i32>>>>,
    group_workers: ThreadPool<(ChunkInput, GroupId), (ChunkOutput, GroupId)>,
    workers: ThreadPool<ChunkInput, ChunkOutput>,
}

type ChunkInput = (Point3<i32>, Arc<ChunkData>, Instant);

type ChunkOutput = (Point3<i32>, (Vec<BlockVertex>, Vec<BlockVertex>), Instant);

impl World {
    pub fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
        textures_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self {
            meshes: Default::default(),
            program: Program::builder()
                .renderer(renderer)
                .shader_desc(read_wgsl("assets/shaders/block.wgsl"))
                .bind_group_layouts(&[
                    player_bind_group_layout,
                    sky_bind_group_layout,
                    textures_bind_group_layout,
                ])
                .immediate_size(BlockImmediates::SIZE)
                .buffers(&[BlockVertex::desc()])
                .cull_mode(wgpu::Face::Back)
                .depth_stencil(wgpu::DepthStencilState {
                    format: DepthBuffer::FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: Default::default(),
                    bias: Default::default(),
                })
                .format(PostProcessor::FORMAT)
                .blend(wgpu::BlendState::ALPHA_BLENDING)
                .build(),
            unloaded: Default::default(),
            groups: Default::default(),
            group_workers: ThreadPool::new(|(input, group_id)| (Self::vertices(input), group_id)),
            workers: ThreadPool::new(Self::vertices),
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
        textures_bind_group: &wgpu::BindGroup,
        depth_view: &wgpu::TextureView,
        frustum: &Frustum,
        intermediate_action: F,
    ) {
        let mut transparent_meshes = vec![];

        {
            let mut render_pass = Self::render_pass(view, encoder, depth_view, true);

            self.program.bind(
                &mut render_pass,
                [player_bind_group, sky_bind_group, textures_bind_group],
            );

            for (&coords, (mesh, _)) in &mut self.meshes {
                if Chunk::bounding_sphere(coords).is_visible(frustum) {
                    if let Some(opaque_part) = mesh.opaque_part() {
                        BlockImmediates::new(coords).set(&mut render_pass);
                        opaque_part.draw(&mut render_pass);
                    }

                    if let Some(transparent_part) = mesh.transparent_part_mut() {
                        transparent_meshes.push((coords, transparent_part));
                    }
                }
            }
        }

        intermediate_action(encoder);

        let mut render_pass = Self::render_pass(view, encoder, depth_view, false);

        self.program.bind(
            &mut render_pass,
            [player_bind_group, sky_bind_group, textures_bind_group],
        );

        transparent_meshes.sort_unstable_by_key(|&(coords, _)| {
            Reverse(utils::magnitude_squared(
                coords,
                utils::chunk_coords(frustum.origin),
            ))
        });

        for (coords, mesh) in transparent_meshes {
            let delta = coords.cast() * Chunk::DIM as f32 - frustum.origin;
            BlockImmediates::new(coords).set(&mut render_pass);
            mesh.draw(renderer, &mut render_pass, |&coords| {
                TotalOrd((coords.coords + delta).magnitude_squared())
            });
        }
    }

    fn send(&self, input: ChunkInput, group_id: Option<GroupId>) {
        if let Some(group_id) = group_id {
            self.group_workers
                .send((input, group_id))
                .unwrap_or_else(|_| unreachable!());
        } else {
            self.workers.send(input).unwrap_or_else(|_| unreachable!());
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
                    entry.insert(vec![output]);
                }
            }
        }
    }

    fn apply_output(&mut self, renderer: &Renderer, output: Result<ChunkOutput, Point3<i32>>) {
        let (coords, (vertices, transparent_vertices), updated_at) = match output {
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
                    if let Some(mesh) = ChunkMesh::new(renderer, &vertices, &transparent_vertices) {
                        *chunk_mesh = mesh;
                    } else {
                        entry.remove();
                    }
                }
            }
            Entry::Vacant(entry) => {
                if let Some(mesh) = ChunkMesh::new(renderer, &vertices, &transparent_vertices) {
                    entry.insert((mesh, updated_at));
                }
            }
        }
    }

    fn vertices((coords, data, updated_at): ChunkInput) -> ChunkOutput {
        (coords, data.vertices(), updated_at)
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
                    self.send((*coords, data.clone(), Instant::now()), *group_id);
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
                    self.send((*coords, data.clone(), Instant::now()), *group_id);
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

enum ChunkMesh {
    Mixed {
        opaque_part: OpaquePart,
        transparent_part: TransparentPart,
    },
    Opaque(OpaquePart),
    Transparent(TransparentPart),
}

type OpaquePart = VertexBuffer<BlockVertex>;

type TransparentPart = TransparentMesh<Point3<f32>, BlockVertex>;

impl ChunkMesh {
    fn new(
        renderer: &Renderer,
        vertices: &[BlockVertex],
        transparent_vertices: &[BlockVertex],
    ) -> Option<Self> {
        match (
            VertexBuffer::try_new(renderer, MemoryState::Immutable(vertices)),
            TransparentMesh::try_new(renderer, transparent_vertices, |v| {
                v.iter()
                    .fold(Point3::default(), |acc, v| acc + v.coords().coords)
                    .cast()
                    / v.len() as f32
            }),
        ) {
            (Some(opaque_part), Some(transparent_part)) => Some(Self::Mixed {
                opaque_part,
                transparent_part,
            }),
            (Some(opaque_part), None) => Some(Self::Opaque(opaque_part)),
            (None, Some(transparent_part)) => Some(Self::Transparent(transparent_part)),
            (None, None) => None,
        }
    }

    fn opaque_part(&self) -> Option<&OpaquePart> {
        if let Self::Mixed { opaque_part, .. } | Self::Opaque(opaque_part) = self {
            Some(opaque_part)
        } else {
            None
        }
    }

    #[rustfmt::skip]
    fn transparent_part_mut(&mut self) -> Option<&mut TransparentPart> {
        if let Self::Mixed { transparent_part, .. } | Self::Transparent(transparent_part) = self {
            Some(transparent_part)
        } else {
            None
        }
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
        face: Face,
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
        data[0].set_bit_range(24, 23, face as u8);
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
