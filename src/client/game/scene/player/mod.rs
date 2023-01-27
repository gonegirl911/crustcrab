pub mod camera;
pub mod frustum;
pub mod projection;

use self::{
    camera::{Camera, CameraController, Changes},
    frustum::Frustum,
    projection::Projection,
};
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::Renderer,
        ClientEvent,
    },
    server::scene::world::{block::Block, chunk::Chunk},
};
use bytemuck::{Pod, Zeroable};
use flume::Sender;
use nalgebra::{point, Matrix4, Point3, Vector3};
use std::{mem, slice, time::Duration};
use winit::event::StartCause;

pub struct Player {
    camera: Camera,
    controller: CameraController,
    projection: Projection,
    render_distance: u32,
    uniform: PlayerUniform,
    is_updated: bool,
}

impl Player {
    pub fn new(renderer @ Renderer { config, .. }: &Renderer) -> Self {
        let aspect = config.width as f32 / config.height as f32;
        let render_distance = 36;
        let zfar = (render_distance * Chunk::DIM) as f32;
        Self {
            camera: Camera::new(point![0.0, 100.0, 0.0], Vector3::z(), Vector3::y()),
            controller: CameraController::new(20.0, 0.4),
            projection: Projection::new(70.0, aspect, 0.1, zfar),
            render_distance: render_distance as u32,
            uniform: PlayerUniform::new(renderer),
            is_updated: true,
        }
    }

    pub fn frustum(&self) -> Frustum {
        Frustum::new(
            self.camera.forward(),
            self.camera.right(),
            self.camera.up(),
            self.camera.origin(),
            self.projection.fovy(),
            self.projection.aspect(),
            self.projection.znear(),
            self.projection.zfar(),
        )
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.uniform.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.uniform.bind_group()
    }
}

impl EventHandler for Player {
    type Context<'a> = (Sender<ClientEvent>, &'a Renderer, Duration);

    fn handle(&mut self, event: &Event, (client_tx, renderer, dt): Self::Context<'_>) {
        self.controller.handle(event, ());
        self.projection.handle(event, ());

        match event {
            Event::NewEvents(StartCause::Init) => {
                client_tx
                    .send(ClientEvent::InitialRenderRequested {
                        player_dir: self.camera.forward(),
                        player_coords: self.camera.origin(),
                        render_distance: self.render_distance,
                    })
                    .unwrap_or_else(|_| unreachable!());
            }
            Event::MainEventsCleared => {
                let changes = self.controller.apply_updates(&mut self.camera, dt);

                if changes.contains(Changes::ROTATED) {
                    client_tx
                        .send(ClientEvent::PlayerOrientationChanged {
                            dir: self.camera.forward(),
                        })
                        .unwrap_or_else(|_| unreachable!());
                }

                if changes.contains(Changes::MOVED) {
                    client_tx
                        .send(ClientEvent::PlayerPositionChanged {
                            coords: self.camera.origin(),
                        })
                        .unwrap_or_else(|_| unreachable!());
                }

                if changes.contains(Changes::BLOCK_DESTROYED) {
                    client_tx
                        .send(ClientEvent::BlockDestroyed)
                        .unwrap_or_else(|_| unreachable!());
                } else if changes.contains(Changes::BLOCK_PLACED) {
                    client_tx
                        .send(ClientEvent::BlockPlaced {
                            block: Block::Grass,
                        })
                        .unwrap_or_else(|_| unreachable!());
                }

                self.is_updated =
                    self.is_updated || changes.intersects(Changes::ROTATED | Changes::MOVED)
            }
            Event::RedrawRequested(_) if self.is_updated => {
                self.uniform.update(
                    renderer,
                    &PlayerUniformData {
                        vp: self.projection.mat() * self.camera.mat(),
                        origin: self.camera.origin(),
                        render_distance: self.render_distance,
                    },
                );
            }
            Event::RedrawEventsCleared => {
                self.is_updated = false;
            }
            _ => {}
        }
    }
}

struct PlayerUniform {
    buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl PlayerUniform {
    fn new(Renderer { device, .. }: &Renderer) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: mem::size_of::<PlayerUniformData>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        Self {
            buffer,
            bind_group_layout,
            bind_group,
        }
    }

    fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    fn update(&self, Renderer { queue, .. }: &Renderer, data: &PlayerUniformData) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(slice::from_ref(data)))
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct PlayerUniformData {
    vp: Matrix4<f32>,
    origin: Point3<f32>,
    render_distance: u32,
}
