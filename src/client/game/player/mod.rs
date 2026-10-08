pub mod camera;
pub mod controller;
pub mod frustum;

use super::gui::Gui;
use crate::{
    client::{
        CLIENT_CONFIG, PlayerEvent,
        event_loop::{Event, EventHandler},
        renderer::{Renderer, Surface, buffer::MemoryState, uniform::Uniform},
        stopwatch::Stopwatch,
    },
    server::{ControlEvent, game::world::chunk::Chunk},
    shared::{color::Float3, pacer::Pacer},
};
use bitflags::bitflags;
use bytemuck::{Pod, Zeroable};
use camera::{Projection, View};
use controller::{Changes, Controller};
use crossbeam_channel::Sender;
use frustum::Frustum;
use nalgebra::{Matrix4, Point3, Vector3};
use serde::Deserialize;
use std::{f32::consts::SQRT_2, mem, time::Duration};
use winit::event::WindowEvent;

pub struct Player {
    pub mut(self) view: View,
    projection: Projection,
    controller: Controller,
    pub mut(self) uniform: Uniform<PlayerUniformData>,
    view_report_pacer: Pacer,
}

impl Player {
    pub fn new(renderer: &Renderer) -> Self {
        let config = &CLIENT_CONFIG.player;
        let view = View::new(Point3::origin(), Vector3::x());
        let projection = Projection::new(config.fovy, 0.0, 0.1, Self::zfar());
        let controller = Controller::new(config.sensitivity);
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
            view_report_pacer: Pacer::new(VIEW_REPORT_GAP),
        }
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

    fn zfar() -> f32 {
        let render_distance = CLIENT_CONFIG.player.render_distance;
        let buffer = 1024.0;
        ((render_distance as u64 + 1) * Chunk::DIM as u64) as f32 * SQRT_2 + buffer
    }
}

impl EventHandler for Player {
    type Context<'a> = (
        &'a Sender<PlayerEvent>,
        &'a Stopwatch,
        &'a Renderer,
        &'a Surface,
        &'a Gui,
    );

    fn handle(
        &mut self,
        event: &Event,
        (player_tx, &Stopwatch { now, dt }, renderer, surface, gui): Self::Context<'_>,
    ) {
        self.controller.handle(event, ());

        match event {
            Event::Resumed => {
                _ = player_tx.send(PlayerEvent::JoinRequested {
                    render_distance: CLIENT_CONFIG.player.render_distance,
                });
            }
            &Event::ControlEvent(ControlEvent::PlayerInitialized { origin, dir, .. }) => {
                self.view = View::new(origin, dir);
                self.controller.external_updates_applied = true;
            }
            Event::WindowEvent(WindowEvent::RedrawRequested) => {
                let changes = self.controller.apply_updates(&mut self.view, dt, now);

                if changes.intersects(Changes::VIEW) && self.view_report_pacer.admit(now) {
                    if changes.contains(Changes::MOVED) {
                        _ = player_tx.send(PlayerEvent::Position {
                            origin: self.view.origin,
                        });
                    }

                    if changes.contains(Changes::ROTATED) {
                        _ = player_tx.send(PlayerEvent::Orientation {
                            dir: self.view.forward,
                        });
                    }
                }

                if surface.is_resized {
                    self.projection.aspect = surface.width() / surface.height();
                }

                if changes.contains(Changes::BLOCK_PLACED) {
                    if let Some(block) = gui.inventory.selected_block() {
                        _ = player_tx.send(PlayerEvent::BlockPlaced(block));
                    }
                } else if changes.contains(Changes::BLOCK_DESTROYED) {
                    _ = player_tx.send(PlayerEvent::BlockDestroyed);
                }

                let external_updates_applied =
                    mem::take(&mut self.controller.external_updates_applied);

                if external_updates_applied
                    || changes.intersects(Changes::VIEW)
                    || surface.is_resized
                {
                    self.uniform.set(
                        renderer,
                        &PlayerUniformData::new(
                            self.projection.mat() * self.view.mat(),
                            self.view.origin,
                            self.view.anchor(),
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

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct PlayerUniformData {
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
        origin: Point3<f64>,
        anchor: Point3<f64>,
        forward: Vector3<f32>,
        znear: f32,
        zfar: f32,
    ) -> Self {
        Self {
            vp,
            inv_vp: vp.try_inverse().unwrap(),
            origin: (origin - anchor).cast().into(),
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
    pub render_distance: u32,
    #[serde(default)]
    features: PlayerFeatures,
}

bitflags! {
    #[derive(Default, Deserialize)]
    struct PlayerFeatures: u8 {
        const DRAWING_MODE = 1 << 0;
    }
}

const VIEW_REPORT_GAP: Duration = Duration::from_millis(16);
