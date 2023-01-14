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
use rustc_hash::FxHashMap;
use std::{mem, sync::Arc, thread};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

pub struct ChunkMeshPool {
    meshes: FxHashMap<Point3<i32>, ChunkMesh>,
    data_tx: Sender<(Point3<i32>, Arc<ChunkData>)>,
    vertices_rx: Receiver<(Point3<i32>, Vec<BlockVertex>)>,
}

impl ChunkMeshPool {
    pub fn new() -> Self {
        let meshes = FxHashMap::default();
        let (data_tx, data_rx) = flume::unbounded::<(_, Arc<ChunkData>)>();
        let (vertices_tx, vertices_rx) = flume::unbounded();

        for _ in 0..num_cpus::get() {
            let data_rx = data_rx.clone();
            let vertices_tx = vertices_tx.clone();
            thread::spawn(move || {
                for (coords, data) in data_rx {
                    vertices_tx
                        .send((coords, data.vertices().collect()))
                        .unwrap_or_else(|_| unreachable!());
                }
            });
        }

        Self {
            meshes,
            data_tx,
            vertices_rx,
        }
    }

    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, frustum: &Frustum) {
        for (coords, mesh) in &self.meshes {
            if Self::bounding_sphere(*coords).is_visible(frustum) {
                mesh.draw(render_pass, *coords);
            }
        }
    }

    fn bounding_sphere(coords: Point3<i32>) -> BoundingSphere {
        BoundingSphere::new(
            coords.map(|c| (c as f32 + 0.5) * Chunk::DIM as f32),
            Chunk::DIM as f32 * 3.0f32.sqrt() / 2.0,
        )
    }
}

impl EventHandler for ChunkMeshPool {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        match event {
            Event::UserEvent(ServerEvent::ChunkUpdated { coords, data }) => {
                if let Some(data) = data {
                    self.data_tx
                        .send((*coords, data.clone()))
                        .unwrap_or_else(|_| unreachable!());
                } else {
                    self.meshes.remove(coords);
                }
            }
            Event::RedrawRequested(_) => {
                for (coords, vertices) in self.vertices_rx.try_iter() {
                    if !vertices.is_empty() {
                        self.meshes
                            .insert(coords, ChunkMesh::new(renderer, &vertices));
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
