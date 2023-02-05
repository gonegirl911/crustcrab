use super::depth_buffer::DepthBuffer;
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        game::player::frustum::{BoundingSphere, Frustum, FrustumCheck},
        renderer::{ImageTexture, Mesh, Program, Renderer, Vertex},
    },
    server::{
        game::scene::world::{
            block::Face,
            chunk::{Chunk, ChunkData},
        },
        ServerEvent,
    },
};
use bytemuck::{Pod, Zeroable};
use flume::{Receiver, Sender};
use nalgebra::{Point2, Point3};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{mem, sync::Arc, thread, time::Instant};

pub struct World {
    meshes: ChunkMeshPool,
    atlas: ImageTexture,
    program: Program,
}

impl World {
    pub fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        clock_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let meshes = ChunkMeshPool::new();
        let atlas = ImageTexture::new(
            renderer,
            include_bytes!("../../../../assets/textures/atlas.png"),
            true,
        );
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../../assets/shaders/block.wgsl"),
            &[BlockVertex::desc()],
            &[
                player_bind_group_layout,
                clock_bind_group_layout,
                atlas.bind_group_layout(),
                sky_bind_group_layout,
            ],
            &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::VERTEX,
                range: 0..mem::size_of::<BlockPushConstants>() as u32,
            }],
            None,
            Some(wgpu::Face::Back),
            Some(wgpu::DepthStencilState {
                format: DepthBuffer::FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
        );
        Self {
            meshes,
            atlas,
            program,
        }
    }

    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        player_bind_group: &'a wgpu::BindGroup,
        clock_bind_group: &'a wgpu::BindGroup,
        sky_bind_group: &'a wgpu::BindGroup,
        frustum: &Frustum,
    ) {
        self.program.draw(
            render_pass,
            [
                player_bind_group,
                clock_bind_group,
                self.atlas.bind_group(),
                sky_bind_group,
            ],
        );
        self.meshes.draw(render_pass, frustum);
    }
}

impl EventHandler for World {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.meshes.handle(event, renderer);
    }
}

struct ChunkMeshPool {
    meshes: FxHashMap<Point3<i32>, (Mesh<BlockVertex>, Instant)>,
    unloaded: FxHashSet<Point3<i32>>,
    data_tx: Sender<(Point3<i32>, Arc<ChunkData>, Instant)>,
    vertices_rx: Receiver<(Point3<i32>, Vec<BlockVertex>, Instant)>,
}

impl ChunkMeshPool {
    fn new() -> Self {
        let meshes = FxHashMap::default();
        let unloaded = FxHashSet::default();
        let (data_tx, data_rx) = flume::unbounded::<(_, Arc<ChunkData>, _)>();
        let (vertices_tx, vertices_rx) = flume::unbounded();

        for _ in 0..num_cpus::get() {
            let data_rx = data_rx.clone();
            let vertices_tx = vertices_tx.clone();
            thread::spawn(move || {
                for (coords, data, updated_at) in data_rx {
                    vertices_tx
                        .send((coords, data.vertices().collect(), updated_at))
                        .unwrap_or_else(|_| unreachable!());
                }
            });
        }

        Self {
            meshes,
            unloaded,
            data_tx,
            vertices_rx,
        }
    }

    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, frustum: &Frustum) {
        for (coords, (mesh, _)) in &self.meshes {
            if Self::bounding_sphere(*coords).is_visible(frustum) {
                render_pass.set_push_constants(
                    wgpu::ShaderStages::VERTEX,
                    0,
                    bytemuck::cast_slice(&[BlockPushConstants::new(*coords)]),
                );
                mesh.draw(render_pass);
            }
        }
    }

    fn bounding_sphere(coords: Point3<i32>) -> BoundingSphere {
        let dim = Chunk::DIM as f32;
        BoundingSphere {
            center: coords.map(|c| c as f32 + 0.5) * dim,
            radius: dim * 3.0f32.sqrt() * 0.5,
        }
    }
}

impl EventHandler for ChunkMeshPool {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        match event {
            Event::UserEvent(event) => match event {
                ServerEvent::ChunkLoaded { coords, data } => {
                    self.unloaded.remove(coords);
                    self.data_tx
                        .send((*coords, data.clone(), Instant::now()))
                        .unwrap_or_else(|_| unreachable!());
                }
                ServerEvent::ChunkUnloaded { coords } => {
                    self.meshes.remove(coords);
                    self.unloaded.insert(*coords);
                }
                ServerEvent::ChunkUpdated { coords, data } => {
                    self.data_tx
                        .send((*coords, data.clone(), Instant::now()))
                        .unwrap_or_else(|_| unreachable!());
                }
                _ => {}
            },
            Event::RedrawRequested(_) => {
                for (coords, vertices, updated_at) in self.vertices_rx.try_iter() {
                    if !self.unloaded.contains(&coords) {
                        if !vertices.is_empty() {
                            self.meshes
                                .entry(coords)
                                .and_modify(|(mesh, last_updated_at)| {
                                    if updated_at > *last_updated_at {
                                        *mesh = Mesh::new(renderer, &vertices);
                                        *last_updated_at = updated_at;
                                    }
                                })
                                .or_insert_with(|| (Mesh::new(renderer, &vertices), updated_at));
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

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct BlockVertex(u32);

impl BlockVertex {
    pub fn new(
        coords: Point3<u8>,
        tex_coords: Point2<u8>,
        atlas_coords: Point2<u8>,
        face: Face,
        ambient_occlusion: u8,
    ) -> Self {
        let mut data = 0;
        data |= coords.x as u32;
        data |= (coords.y as u32) << 5;
        data |= (coords.z as u32) << 10;
        data |= (tex_coords.x as u32) << 15;
        data |= (tex_coords.y as u32) << 16;
        data |= (atlas_coords.x as u32) << 17;
        data |= (atlas_coords.y as u32) << 21;
        data |= (face as u32) << 25;
        data |= (ambient_occlusion as u32) << 27;
        Self(data)
    }
}

impl Vertex for BlockVertex {
    const ATTRIBS: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![0 => Uint32];
}