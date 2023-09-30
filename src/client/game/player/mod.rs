pub mod camera;
pub mod frustum;

use self::{
    camera::{Changes, Controller, Projection, View},
    frustum::Frustum,
};
use super::gui::inventory::Inventory;
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
use winit::event::StartCause;

pub struct Player {
    view: View,
    projection: Projection,
    controller: Controller,
    uniform: Uniform<PlayerUniformData>,
}

impl Player {
    pub fn new(renderer @ Renderer { config, .. }: &Renderer) -> Self {
        let state = &CLIENT_CONFIG.player;
        let view = View::new(state.origin, Vector3::x());
        let aspect = config.width as f32 / config.height as f32;
        let projection = Projection::new(state.fovy, aspect, 0.1, state.zfar());
        let controller = Controller::new(state.speed, state.sensitivity);
        let uniform = Uniform::new(
            renderer,
            MemoryState::Mutable(&Self::data(&view, &projection)),
            wgpu::ShaderStages::VERTEX_FRAGMENT,
        );
        Self {
            view,
            controller,
            projection,
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

    fn data(view: &View, projection: &Projection) -> PlayerUniformData {
        PlayerUniformData::new(
            view.mat(),
            projection.mat(),
            view.origin,
            view.forward,
            projection.znear,
            projection.zfar,
        )
    }
}

impl EventHandler for Player {
    type Context<'a> = (Sender<ClientEvent>, &'a Renderer, &'a Inventory, Duration);

    fn handle(&mut self, event: &Event, (client_tx, renderer, inventory, dt): Self::Context<'_>) {
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
            Event::MainEventsCleared => {
                let changes =
                    self.controller
                        .apply_updates(&mut self.view, &mut self.projection, dt);

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

                if changes.contains(Changes::BLOCK_PLACED) {
                    if let Some(block) = inventory.selected_block() {
                        client_tx
                            .send(ClientEvent::BlockPlaced { block })
                            .unwrap_or_else(|_| unreachable!());
                    }
                } else if changes.contains(Changes::BLOCK_DESTROYED) {
                    client_tx
                        .send(ClientEvent::BlockDestroyed)
                        .unwrap_or_else(|_| unreachable!());
                }

                if changes.intersects(Changes::MATRIX_CHANGES) {
                    self.uniform
                        .set(renderer, &Self::data(&self.view, &self.projection));
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
    inv_v: Matrix4<f32>,
    inv_p: Matrix4<f32>,
    origin: Float3,
    forward: Vector3<f32>,
    render_distance: u32,
    znear: f32,
    zfar: f32,
    padding: [f32; 2],
}

impl PlayerUniformData {
    fn new(
        v: Matrix4<f32>,
        p: Matrix4<f32>,
        origin: Point3<f32>,
        forward: Vector3<f32>,
        znear: f32,
        zfar: f32,
    ) -> Self {
        Self {
            vp: p * v,
            inv_v: v.try_inverse().unwrap_or_else(|| unreachable!()),
            inv_p: p.try_inverse().unwrap_or_else(|| unreachable!()),
            origin: origin.into(),
            forward,
            render_distance: CLIENT_CONFIG.player.render_distance,
            znear,
            zfar,
            padding: [0.0; 2],
        }
    }
}

#[derive(Deserialize)]
pub struct PlayerConfig {
    origin: Point3<f32>,
    fovy: f32,
    speed: f32,
    sensitivity: f32,
    pub render_distance: u32,
}

impl PlayerConfig {
    fn zfar(&self) -> f32 {
        1000.0 + SQRT_2 * (self.render_distance + 1) as f32 * Chunk::DIM as f32
    }
}
