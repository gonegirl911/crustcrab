pub mod camera;
pub mod frustum;

use self::{
    camera::{Camera, Changes, Controller, Projection},
    frustum::Frustum,
};
use super::gui::Gui;
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{Renderer, Uniform},
        ClientEvent,
    },
    server::game::world::chunk::Chunk,
};
use bytemuck::{Pod, Zeroable};
use flume::Sender;
use nalgebra::{point, Matrix4, Point3, Vector3};
use std::time::Duration;
use winit::event::StartCause;

pub struct Player {
    camera: Camera,
    projection: Projection,
    controller: Controller,
    uniform: Uniform<PlayerUniformData>,
    is_updated: bool,
}

impl Player {
    pub fn new(renderer @ Renderer { config, .. }: &Renderer, gui: &Gui) -> Self {
        let camera = Camera::new(point![0.0, 100.0, 0.0], Vector3::z(), Vector3::y());
        let aspect = config.width as f32 / config.height as f32;
        let zfar = (gui.render_distance() * Chunk::DIM as u32) as f32;
        let projection = Projection::new(90.0, aspect, 0.1, zfar);
        let controller = Controller::new(25.0, 0.15);
        let uniform = Uniform::new(renderer, wgpu::ShaderStages::VERTEX);
        Self {
            camera,
            controller,
            projection,
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
}

impl EventHandler for Player {
    type Context<'a> = (Sender<ClientEvent>, &'a Renderer, &'a Gui, Duration);

    fn handle(&mut self, event: &Event, (client_tx, renderer, gui, dt): Self::Context<'_>) {
        self.controller.handle(event, renderer);

        match event {
            Event::NewEvents(StartCause::Init) => {
                client_tx
                    .send(ClientEvent::InitialRenderRequested {
                        player_dir: self.camera.forward(),
                        player_coords: self.camera.origin(),
                        render_distance: gui.render_distance(),
                    })
                    .unwrap_or_else(|_| unreachable!());
            }
            Event::MainEventsCleared => {
                let changes =
                    self.controller
                        .apply_updates(&mut self.camera, &mut self.projection, dt);

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
                            block: gui.selected_block(),
                        })
                        .unwrap_or_else(|_| unreachable!());
                }

                self.is_updated = self.is_updated || changes.intersects(Changes::MATRIX_CHANGES);
            }
            Event::RedrawRequested(_) if self.is_updated => {
                self.uniform.write(
                    renderer,
                    &PlayerUniformData::new(
                        self.projection.mat() * self.camera.mat(),
                        self.camera.origin(),
                        gui.render_distance(),
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
