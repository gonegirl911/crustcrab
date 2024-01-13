use super::player::frustum::{Cullable, Frustum};
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{
            buffer::{MemoryState, Vertex, VertexBuffer},
            effect::PostProcessor,
            program::{Program, PushConstants},
            texture::screen::DepthBuffer,
            utils::{TotalOrd, TransparentMesh},
            Renderer,
        },
    },
    server::{
        game::world::{
            block::{data::Face, BlockLight},
            chunk::Chunk,
            ChunkData,
        },
        ServerEvent,
    },
    shared::{
        pool::ThreadPool,
        ray::{Intersectable, Ray},
        utils,
    },
};
use bitfield::{BitRange, BitRangeMut};
use bytemuck::{Pod, Zeroable};
use nalgebra::{point, Point2, Point3};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{cmp::Reverse, collections::hash_map::Entry, sync::Arc, time::Instant};
use winit::event::WindowEvent;

pub struct World {
    meshes: FxHashMap<Point3<i32>, ChunkMesh>,
    program: Program,
    unloaded: FxHashSet<Point3<i32>>,
    priority_workers: ThreadPool<ChunkInput, ChunkOutput>,
    workers: ThreadPool<ChunkInput, ChunkOutput>,
}

type ChunkMesh = (
    VertexBuffer<BlockVertex>,
    Option<TransparentMesh<Point3<f32>, BlockVertex>>,
    Instant,
);

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
            program: Program::new(
                renderer,
                wgpu::include_wgsl!("../../../assets/shaders/block.wgsl"),
                &[BlockVertex::desc()],
                &[
                    player_bind_group_layout,
                    sky_bind_group_layout,
                    textures_bind_group_layout,
                ],
                &[BlockPushConstants::range()],
                Some(wgpu::Face::Back),
                Some(wgpu::DepthStencilState {
                    format: DepthBuffer::FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                PostProcessor::FORMAT,
                Some(wgpu::BlendState::ALPHA_BLENDING),
            ),
            unloaded: Default::default(),
            priority_workers: ThreadPool::new(Self::vertices),
            workers: ThreadPool::new(Self::vertices),
        }
    }

    #[allow(clippy::too_many_arguments)]
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

            for (&coords, (buffer, transparent_mesh, _)) in &mut self.meshes {
                if Chunk::bounding_sphere(coords).is_visible(frustum) {
                    BlockPushConstants::new(coords).set(&mut render_pass);
                    buffer.draw(&mut render_pass);

                    if let Some(mesh) = transparent_mesh {
                        transparent_meshes.push((coords, mesh));
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
            let origin = Self::block_coords(Self::origin(frustum.origin, coords), coords);
            BlockPushConstants::new(coords).set(&mut render_pass);
            mesh.draw(renderer, &mut render_pass, |&coords| {
                TotalOrd((coords - origin).magnitude_squared())
            });
        }
    }

    fn workers(&self, is_important: bool) -> &ThreadPool<ChunkInput, ChunkOutput> {
        if is_important {
            &self.priority_workers
        } else {
            &self.workers
        }
    }

    fn vertices((coords, data, updated_at): ChunkInput) -> ChunkOutput {
        (coords, data.vertices(), updated_at)
    }

    fn transparent_mesh(
        renderer: &Renderer,
        vertices: &[BlockVertex],
    ) -> Option<TransparentMesh<Point3<f32>, BlockVertex>> {
        TransparentMesh::new_non_empty(renderer, vertices, |v| {
            v.iter()
                .copied()
                .map(BlockVertex::coords)
                .fold(Point3::default(), |accum, c| accum + c.coords)
                .cast()
                / v.len() as f32
        })
    }

    fn render_pass<'a>(
        view: &'a wgpu::TextureView,
        encoder: &'a mut wgpu::CommandEncoder,
        depth_view: &'a wgpu::TextureView,
        is_initial: bool,
    ) -> wgpu::RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
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

    fn origin(origin: Point3<f32>, coords: Point3<i32>) -> Point3<f32> {
        let aabb = Chunk::bounding_box(coords);
        if aabb.contains(origin) {
            origin
        } else {
            aabb.intersection(Ray::look_at(origin, aabb.circumcenter()))
                .unwrap_or_else(|| unreachable!())
        }
    }

    fn block_coords(coords: Point3<f32>, chunk_coords: Point3<i32>) -> Point3<f32> {
        coords - chunk_coords.coords.cast() * Chunk::DIM as f32
    }
}

impl EventHandler for World {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        match event {
            Event::UserEvent(event) => match event {
                ServerEvent::ChunkLoaded {
                    coords,
                    data,
                    is_important,
                } => {
                    self.unloaded.remove(coords);
                    self.workers(*is_important)
                        .send((*coords, data.clone(), Instant::now()))
                        .unwrap_or_else(|_| unreachable!());
                }
                ServerEvent::ChunkUnloaded { coords } => {
                    self.meshes.remove(coords);
                    self.unloaded.insert(*coords);
                }
                ServerEvent::ChunkUpdated {
                    coords,
                    data,
                    is_important,
                } => {
                    self.workers(*is_important)
                        .send((*coords, data.clone(), Instant::now()))
                        .unwrap_or_else(|_| unreachable!());
                }
                _ => {}
            },
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                for (coords, (vertices, transparent_vertices), updated_at) in
                    self.priority_workers.drain().chain(self.workers.drain())
                {
                    if !self.unloaded.contains(&coords) {
                        if !vertices.is_empty() || !transparent_vertices.is_empty() {
                            match self.meshes.entry(coords) {
                                Entry::Occupied(entry) => {
                                    let (_, _, last_updated_at) = *entry.get();
                                    if last_updated_at < updated_at {
                                        *entry.into_mut() = (
                                            VertexBuffer::new(
                                                renderer,
                                                MemoryState::Immutable(&vertices),
                                            ),
                                            Self::transparent_mesh(renderer, &transparent_vertices),
                                            updated_at,
                                        );
                                    }
                                }
                                Entry::Vacant(entry) => {
                                    entry.insert((
                                        VertexBuffer::new(
                                            renderer,
                                            MemoryState::Immutable(&vertices),
                                        ),
                                        Self::transparent_mesh(renderer, &transparent_vertices),
                                        updated_at,
                                    ));
                                }
                            }
                        } else {
                            self.meshes.remove(&coords);
                        }
                    }
                }
            }
            _ => {}
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
    const ATTRIBS: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![0 => Uint32x2];
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct BlockPushConstants {
    chunk_coords: Point3<f32>,
}

impl BlockPushConstants {
    fn new(chunk_coords: Point3<i32>) -> Self {
        Self {
            chunk_coords: chunk_coords.cast(),
        }
    }
}

impl PushConstants for BlockPushConstants {
    const STAGES: wgpu::ShaderStages = wgpu::ShaderStages::VERTEX;
}
