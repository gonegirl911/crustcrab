use super::block::BlockVertex;
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        game::scene::player::frustum::{BoundingSphere, Frustum, FrustumCheck},
        renderer::Renderer,
    },
    server::{
        scene::world::chunk::{Chunk, ChunkData},
        ServerEvent,
    },
};
use bytemuck::{Pod, Zeroable};
use flume::{Receiver, Sender};
use nalgebra::Point3;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{mem, sync::Arc, thread, time::Instant};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

pub struct ChunkMeshPool {
    meshes: FxHashMap<Point3<i32>, (ChunkMesh, Instant)>,
    unloaded: FxHashSet<Point3<i32>>,
    selected_block: Option<Point3<i32>>,
    data_tx: Sender<(Point3<i32>, Arc<ChunkData>, Instant)>,
    vertices_rx: Receiver<(Point3<i32>, Vec<BlockVertex>, Instant)>,
}

impl ChunkMeshPool {
    pub fn new() -> Self {
        let meshes = FxHashMap::default();
        let unloaded = FxHashSet::default();
        let selected_block = None;
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
            selected_block,
            data_tx,
            vertices_rx,
        }
    }

    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, frustum: &Frustum) {
        for (coords, (mesh, _)) in &self.meshes {
            if Self::bounding_sphere(*coords).is_visible(frustum) {
                mesh.draw(render_pass, *coords);
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
                ServerEvent::BlockSelected { coords } => {
                    self.selected_block = *coords;
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
                                        *mesh = ChunkMesh::new(renderer, &vertices);
                                        *last_updated_at = updated_at;
                                    }
                                })
                                .or_insert_with(|| {
                                    (ChunkMesh::new(renderer, &vertices), updated_at)
                                });
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

struct ChunkMesh {
    vertex_buffer: wgpu::Buffer,
}

impl ChunkMesh {
    fn new(Renderer { device, .. }: &Renderer, vertices: &[BlockVertex]) -> Self {
        Self {
            vertex_buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        }
    }

    fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, coords: Point3<i32>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_push_constants(
            wgpu::ShaderStages::VERTEX,
            0,
            bytemuck::cast_slice(&[PushConstants::new(coords)]),
        );
        render_pass.draw(0..self.len(), 0..1);
    }

    fn len(&self) -> u32 {
        (self.vertex_buffer.size() / mem::size_of::<BlockVertex>() as u64) as u32
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct PushConstants {
    chunk_coords: Point3<f32>,
}

impl PushConstants {
    fn new(chunk_coords: Point3<i32>) -> Self {
        Self {
            chunk_coords: chunk_coords.cast(),
        }
    }
}
