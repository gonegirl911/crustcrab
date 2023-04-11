pub mod camera;
pub mod frustum;

use self::{
    camera::{Changes, Controller, Projection, View},
    frustum::Frustum,
};
use super::gui::Gui;
use crate::client::{
    event_loop::{Event, EventHandler},
    renderer::{uniform::Uniform, Renderer},
    ClientEvent,
};
use bytemuck::{Pod, Zeroable};
use flume::Sender;
use nalgebra::{vector, Matrix4, Point3, Vector3};
use std::time::Duration;
use winit::event::StartCause;

pub struct Player {
    view: View,
    projection: Projection,
    controller: Controller,
    uniform: Uniform<PlayerUniformData>,
    is_updated: bool,
}

impl Player {
    pub const WORLD_UP: Vector3<f32> = vector![0.0, 1.0, 0.0];

    pub fn new(renderer @ Renderer { config, .. }: &Renderer, gui: &Gui) -> Self {
        let view = View::new(gui.origin(), Vector3::x());
        let aspect = config.width as f32 / config.height as f32;
        let projection = Projection::new(gui.fovy(), aspect, 0.1, gui.zfar());
        let controller = Controller::new(gui.speed(), gui.sensitivity());
        let uniform = Uniform::new(renderer, wgpu::ShaderStages::VERTEX_FRAGMENT);
        Self {
            view,
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
            self.view.origin(),
            self.view.forward(),
            self.view.right(),
            self.view.up(),
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
        self.controller.handle(event, ());

        match event {
            Event::NewEvents(StartCause::Init) => {
                client_tx
                    .send(ClientEvent::InitialRenderRequested {
                        origin: self.view.origin(),
                        dir: self.view.forward(),
                        render_distance: gui.render_distance(),
                    })
                    .unwrap_or_else(|_| unreachable!());
            }
            Event::MainEventsCleared => {
                let changes =
                    self.controller
                        .apply_updates(&mut self.view, &mut self.projection, dt);

                if changes.contains(Changes::MOVED) {
                    client_tx
                        .send(ClientEvent::PlayerPositionChanged {
                            origin: self.view.origin(),
                        })
                        .unwrap_or_else(|_| unreachable!());
                }

                if changes.contains(Changes::ROTATED) {
                    client_tx
                        .send(ClientEvent::PlayerOrientationChanged {
                            dir: self.view.forward(),
                        })
                        .unwrap_or_else(|_| unreachable!());
                }

                if changes.contains(Changes::BLOCK_PLACED) {
                    if let Some(block) = gui.selected_block() {
                        client_tx
                            .send(ClientEvent::BlockPlaced { block })
                            .unwrap_or_else(|_| unreachable!());
                    }
                } else if changes.contains(Changes::BLOCK_DESTROYED) {
                    client_tx
                        .send(ClientEvent::BlockDestroyed)
                        .unwrap_or_else(|_| unreachable!());
                }

                self.is_updated = self.is_updated || changes.intersects(Changes::MATRIX_CHANGES);
            }
            Event::RedrawRequested(_) if self.is_updated => {
                self.uniform.write(
                    renderer,
                    &PlayerUniformData::new(
                        self.view.mat(),
                        self.projection.mat(),
                        self.view.origin(),
                        self.projection.znear(),
                        self.projection.zfar(),
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
    inv_v: Matrix4<f32>,
    inv_p: Matrix4<f32>,
    origin: Point3<f32>,
    znear: f32,
    zfar: f32,
    padding: [f32; 3],
}

impl PlayerUniformData {
    fn new(v: Matrix4<f32>, p: Matrix4<f32>, origin: Point3<f32>, znear: f32, zfar: f32) -> Self {
        Self {
            vp: p * v,
            inv_v: v.try_inverse().unwrap_or_else(|| unreachable!()),
            inv_p: p.try_inverse().unwrap_or_else(|| unreachable!()),
            origin,
            znear,
            zfar,
            padding: [0.0; 3],
        }
    }
}
