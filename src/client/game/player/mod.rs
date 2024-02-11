pub mod camera;
pub mod frustum;

use self::{
    camera::{Changes, Controller, Projection, View},
    frustum::Frustum,
};
use super::gui::Gui;
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{buffer::MemoryState, uniform::Uniform, Renderer},
        ClientEvent, CLIENT_CONFIG,
    },
    server::game::world::chunk::Chunk,
    shared::color::Float3,
};
use bytemuck::{Pod, Zeroable};
use flume::Sender;
use nalgebra::{Matrix4, Point3, Vector3};
use serde::Deserialize;
use std::{f32::consts::SQRT_2, time::Duration};
use winit::event::{StartCause, WindowEvent};

pub struct Player {
    view: View,
    projection: Projection,
    controller: Controller,
    uniform: Uniform<PlayerUniformData>,
}

impl Player {
    pub fn new(renderer: &Renderer) -> Self {
        let config = &CLIENT_CONFIG.player;
        let view = View::new(config.origin, Vector3::x());
        let projection = Projection::new(config.fovy, 0.0, 0.1, config.zfar());
        let controller = Controller::new(config.speed, config.sensitivity);
        let uniform = Uniform::new(
            renderer,
            MemoryState::UNINIT,
            wgpu::ShaderStages::VERTEX_FRAGMENT,
        );
        Self {
            view,
            projection,
            controller,
            uniform,
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
            self.view.origin,
            self.view.forward,
            self.view.right,
            self.view.up,
            self.projection.fovy,
            self.projection.aspect,
            self.projection.znear,
            self.projection.zfar,
        )
    }
}

impl EventHandler for Player {
    type Context<'a> = (&'a Sender<ClientEvent>, &'a Renderer, &'a Gui, Duration);

    fn handle(&mut self, event: &Event, (client_tx, renderer, gui, dt): Self::Context<'_>) {
        self.controller.handle(event, ());

        match event {
            Event::NewEvents(StartCause::Init) => {
                client_tx
                    .send(ClientEvent::InitialRenderRequested {
                        origin: self.view.origin,
                        dir: self.view.forward,
                        render_distance: CLIENT_CONFIG.player.render_distance,
                    })
                    .unwrap_or_else(|_| unreachable!());
            }
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                let changes = self.controller.apply_updates(&mut self.view, dt);

                if changes.contains(Changes::MOVED) {
                    client_tx
                        .send(ClientEvent::PlayerPositionChanged {
                            origin: self.view.origin,
                        })
                        .unwrap_or_else(|_| unreachable!());
                }

                if changes.contains(Changes::ROTATED) {
                    client_tx
                        .send(ClientEvent::PlayerOrientationChanged {
                            dir: self.view.forward,
                        })
                        .unwrap_or_else(|_| unreachable!());
                }

                if renderer.is_resized {
                    self.projection.aspect = renderer.aspect();
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

                if changes.intersects(Changes::VIEW) || renderer.is_resized {
                    self.uniform.set(
                        renderer,
                        &PlayerUniformData::new(
                            self.projection.mat() * self.view.mat(),
                            self.view.origin,
                            self.view.forward,
                            self.projection.znear,
                            self.projection.zfar,
                        ),
                    );
                }
            }
            _ => {}
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct PlayerUniformData {
    vp: Matrix4<f32>,
    inv_vp: Matrix4<f32>,
    origin: Float3,
    forward: Vector3<f32>,
    render_distance: u32,
    znear: f32,
    zfar: f32,
    padding: [f32; 2],
}

impl PlayerUniformData {
    fn new(
        vp: Matrix4<f32>,
        origin: Point3<f32>,
        forward: Vector3<f32>,
        znear: f32,
        zfar: f32,
    ) -> Self {
        Self {
            vp,
            inv_vp: vp.try_inverse().unwrap_or_else(|| unreachable!()),
            origin: origin.into(),
            forward,
            render_distance: CLIENT_CONFIG.player.render_distance,
            znear,
            zfar,
            padding: Default::default(),
        }
    }
}

#[derive(Deserialize)]
pub struct PlayerConfig {
    origin: Point3<f32>,
    fovy: f32,
    speed: f32,
    sensitivity: f32,
    render_distance: u32,
}

impl PlayerConfig {
    pub fn render_distance(&self) -> u64 {
        self.render_distance as u64 * Chunk::DIM as u64
    }

    fn zfar(&self) -> f32 {
        1000.0 + SQRT_2 * ((self.render_distance + 1) as u64 * Chunk::DIM as u64) as f32
    }
}
