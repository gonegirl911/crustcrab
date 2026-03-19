pub mod camera;
pub mod frustum;

use super::gui::Gui;
use crate::{
    client::{
        CLIENT_CONFIG, ClientEvent,
        event_loop::{Event, EventHandler},
        renderer::{Renderer, Surface, buffer::MemoryState, uniform::Uniform},
    },
    server::{ServerEvent, game::world::chunk::Chunk},
    shared::color::Float3,
};
use bitflags::bitflags;
use bytemuck::{Pod, Zeroable};
use camera::{Changes, Controller, Projection, View};
use crossbeam_channel::Sender;
use frustum::Frustum;
use nalgebra::{Matrix4, Point3, Vector3};
use serde::Deserialize;
use std::{f32::consts::SQRT_2, time::Duration};
use winit::event::WindowEvent;

pub struct Player {
    view: View,
    projection: Projection,
    controller: Controller,
    uniform: Uniform<PlayerUniformData>,
}

impl Player {
    pub fn new(renderer: &Renderer) -> Self {
        let config = &CLIENT_CONFIG.player;
        let view = View::new(Default::default(), Vector3::x());
        let projection = Projection::new(config.fovy, 0.0, 0.1, config.zfar());
        let controller = Controller::new(0.0, config.sensitivity);
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
    type Context<'a> = (
        &'a Sender<ClientEvent>,
        &'a Renderer,
        &'a Surface,
        &'a Gui,
        Duration,
    );

    fn handle(
        &mut self,
        event: &Event,
        (client_tx, renderer, surface, gui, dt): Self::Context<'_>,
    ) {
        self.controller.handle(event, ());

        match event {
            Event::Resumed => {
                _ = client_tx.send(ClientEvent::PlayerConnected {
                    render_distance: CLIENT_CONFIG.player.render_distance,
                });
            }
            &Event::ServerEvent(ServerEvent::PlayerInitialized { origin, dir, .. }) => {
                self.view = View::new(origin, dir);
            }
            Event::WindowEvent(WindowEvent::RedrawRequested) => {
                let changes = self.controller.apply_updates(&mut self.view, dt);

                if changes.contains(Changes::MOVED) {
                    _ = client_tx.send(ClientEvent::PlayerPositionChanged {
                        origin: self.view.origin,
                    });
                }

                if changes.contains(Changes::ROTATED) {
                    _ = client_tx.send(ClientEvent::PlayerOrientationChanged {
                        dir: self.view.forward,
                    });
                }

                if surface.is_resized {
                    self.projection.aspect = surface.width() / surface.height();
                }

                if changes.contains(Changes::BLOCK_PLACED) {
                    if let Some(block) = gui.selected_block() {
                        _ = client_tx.send(ClientEvent::BlockPlaced(block));
                    }
                } else if changes.contains(Changes::BLOCK_DESTROYED) {
                    _ = client_tx.send(ClientEvent::BlockDestroyed);
                }

                if changes.intersects(Changes::VIEW) || surface.is_resized {
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
    fovy: f32,
    sensitivity: f32,
    render_distance: u32,
    #[serde(default)]
    features: PlayerFeatures,
}

impl PlayerConfig {
    pub fn render_distance(&self) -> u64 {
        self.render_distance as u64 * Chunk::DIM as u64
    }

    fn zfar(&self) -> f32 {
        1000.0 + SQRT_2 * ((self.render_distance + 1) as u64 * Chunk::DIM as u64) as f32
    }
}

bitflags! {
    #[derive(Default, Deserialize)]
    struct PlayerFeatures: u8 {
        const DRAWING_MODE = 1 << 0;
    }
}
