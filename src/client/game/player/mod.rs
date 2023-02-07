pub mod camera;
pub mod frustum;
pub mod gui;
pub mod projection;

use self::{
    camera::{Camera, CameraController, Changes},
    frustum::Frustum,
    gui::Gui,
    projection::Projection,
};
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{Renderer, Uniform},
        ClientEvent,
    },
    server::game::scene::world::chunk::Chunk,
};
use bytemuck::{Pod, Zeroable};
use flume::Sender;
use nalgebra::{point, Matrix4, Point3, Vector3};
use std::time::Duration;
use winit::event::StartCause;

pub struct Player {
    gui: Gui,
    camera: Camera,
    controller: CameraController,
    projection: Projection,
    uniform: Uniform<PlayerUniformData>,
    is_updated: bool,
}

impl Player {
    pub fn new(
        renderer @ Renderer { config, .. }: &Renderer,
        output_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let gui = Gui::new(renderer, output_bind_group_layout);
        let camera = Camera::new(point![0.0, 100.0, 0.0], Vector3::z(), Vector3::y());
        let controller = CameraController::new(25.0, 0.25);
        let aspect = config.width as f32 / config.height as f32;
        let zfar = (gui.render_distance() * Chunk::DIM as u32) as f32;
        let projection = Projection::new(90.0, aspect, 0.1, zfar);
        let uniform = Uniform::new(renderer, wgpu::ShaderStages::VERTEX);
        Self {
            camera,
            controller,
            projection,
            gui,
            uniform,
            is_updated: true,
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.uniform.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.uniform.bind_group()
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

    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        output_bind_group: &wgpu::BindGroup,
    ) {
        self.gui.draw(view, encoder, output_bind_group);
    }
}

impl EventHandler for Player {
    type Context<'a> = (Sender<ClientEvent>, &'a Renderer, Duration);

    fn handle(&mut self, event: &Event, (client_tx, renderer, dt): Self::Context<'_>) {
        self.gui.handle(event, renderer);
        self.controller.handle(event, ());
        self.projection.handle(event, ());

        match event {
            Event::NewEvents(StartCause::Init) => {
                client_tx
                    .send(ClientEvent::InitialRenderRequested {
                        player_dir: self.camera.forward(),
                        player_coords: self.camera.origin(),
                        render_distance: self.gui.render_distance(),
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
                            block: self.gui.selected_block(),
                        })
                        .unwrap_or_else(|_| unreachable!());
                }

                self.is_updated =
                    self.is_updated || changes.intersects(Changes::ROTATED | Changes::MOVED);
            }
            Event::RedrawRequested(_) if self.is_updated => {
                self.uniform.update(
                    renderer,
                    &PlayerUniformData::new(
                        self.projection.mat() * self.camera.mat(),
                        self.camera.origin(),
                        self.gui.render_distance(),
                    ),
                );
            }
            Event::RedrawEventsCleared => {
                self.is_updated = false;
            }
            _ => {}
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct PlayerUniformData {
    vp: Matrix4<f32>,
    origin: Point3<f32>,
    render_distance: u32,
}

impl PlayerUniformData {
    fn new(vp: Matrix4<f32>, origin: Point3<f32>, render_distance: u32) -> Self {
        Self {
            vp,
            origin,
            render_distance,
        }
    }
}
