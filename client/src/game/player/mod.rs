pub(super) mod camera;
mod controller;
mod frustum;

use crate::{
    CONFIG,
    event::{self, Event},
    renderer::{Renderer, buffer::MemoryState, surface::Surface, types::Float3, uniform::Uniform},
    stopwatch::Stopwatch,
};
use base::{flow::pacer::Pacer, position::Position3};
use bytemuck::{Pod, Zeroable};
use camera::{Projection, View};
use controller::{Changes, Controller};
use crossbeam_channel::Sender;
use domain::{
    game::world::{chunk::Chunk, coords},
    protocol::{control, player},
};
use frustum::Frustum;
use nalgebra::{Matrix4, Point3, Vector3};
use serde::Deserialize;
use std::{f32::consts::SQRT_2, mem, sync::LazyLock, time::Duration};
use winit::event::WindowEvent;

pub(super) struct Player {
    view: View,
    projection: Projection,
    controller: Controller,
    pub(super) uniform: Uniform<UniformData>,
    pending_view_report: Changes,
    view_report_pacer: Pacer,
}

impl Player {
    const VIEW_REPORT_GAP: Duration = Duration::from_nanos(1000000000 / 60);

    pub(super) fn new(renderer: &Renderer) -> Self {
        let view = View::new(Default::default(), Vector3::x());
        let projection = Projection::new(CONFIG.player.fovy, 0.0, 0.1, *ZFAR);
        let controller = Controller::new(CONFIG.player.sensitivity);
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
            pending_view_report: Default::default(),
            view_report_pacer: Pacer::new(Self::VIEW_REPORT_GAP),
        }
    }

    pub(super) fn render_camera(&self) -> RenderCamera {
        let origin = self.view.origin;
        let anchor = self.anchor();
        let local = origin.displacement_from(anchor).into();
        let frustum = self.frustum();
        RenderCamera {
            origin,
            anchor,
            local,
            frustum,
        }
    }

    fn anchor(&self) -> Point3<i64> {
        coords::from_parts(coords::chunk(self.view.origin.coords), Default::default())
    }

    fn frustum(&self) -> Frustum {
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

impl event::Handler for Player {
    type Context<'a> = (
        &'a Sender<player::Event>,
        &'a Stopwatch,
        &'a Renderer,
        &'a Surface,
    );

    fn handle(
        &mut self,
        event: &Event,
        (player_tx, &Stopwatch { now, dt }, renderer, surface): Self::Context<'_>,
    ) {
        self.controller.handle(event, ());

        match event {
            Event::Init => {
                _ = player_tx.send(player::Event::JoinRequested {
                    render_distance: CONFIG.player.render_distance,
                });
            }
            &Event::Control(control::Event::PlayerInitialized { origin, dir, .. }) => {
                self.view = View::new(origin, dir);
                self.controller.external_updates_applied = true;
                self.pending_view_report = Default::default();
            }
            Event::WindowEvent(WindowEvent::RedrawRequested) => {
                let changes = self.controller.apply_updates(&mut self.view, dt);

                self.pending_view_report |= changes & Changes::VIEW;

                if !self.pending_view_report.is_empty() && self.view_report_pacer.admit(now) {
                    if self.pending_view_report.contains(Changes::MOVED) {
                        _ = player_tx.send(player::Event::Position {
                            origin: self.view.origin,
                        });
                    }

                    if self.pending_view_report.contains(Changes::ROTATED) {
                        _ = player_tx.send(player::Event::Orientation {
                            dir: self.view.forward,
                        });
                    }

                    self.pending_view_report = Default::default();
                }

                if surface.is_resized() {
                    self.projection.aspect =
                        surface.config.width as f32 / surface.config.height as f32;
                }

                if changes.contains(Changes::BLOCK_PLACED) {
                    if let Some(block) = self.controller.inventory.selected_block() {
                        _ = player_tx.send(player::Event::BlockPlaced(block));
                    }
                } else if changes.contains(Changes::BLOCK_DESTROYED) {
                    _ = player_tx.send(player::Event::BlockDestroyed);
                }

                let external_updates_applied =
                    mem::take(&mut self.controller.external_updates_applied);

                if external_updates_applied
                    || changes.intersects(Changes::VIEW)
                    || surface.is_resized()
                {
                    self.uniform.set(
                        renderer,
                        &UniformData::new(
                            self.projection.mat() * self.view.mat(),
                            self.view.origin.displacement_from(self.anchor()).into(),
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

pub(super) struct RenderCamera {
    pub(super) origin: Position3,
    pub(super) anchor: Point3<i64>,
    pub(super) local: Point3<f32>,
    pub(super) frustum: Frustum,
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub(super) struct UniformData {
    vp: Matrix4<f32>,
    inv_vp: Matrix4<f32>,
    origin: Float3,
    forward: Vector3<f32>,
    znear: f32,
    zfar: f32,
    render_distance: u32,
    padding: [f32; 2],
}

impl UniformData {
    fn new(
        vp: Matrix4<f32>,
        origin: Point3<f32>,
        forward: Vector3<f32>,
        znear: f32,
        zfar: f32,
    ) -> Self {
        Self {
            vp,
            inv_vp: vp.try_inverse().unwrap(),
            origin: origin.into(),
            forward,
            znear,
            zfar,
            render_distance: CONFIG.player.render_distance,
            padding: Default::default(),
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct Config {
    fovy: f32,
    sensitivity: f32,
    pub(super) render_distance: u32,
}

static ZFAR: LazyLock<f32> = LazyLock::new(|| {
    let render_distance = CONFIG.player.render_distance;
    let buffer = 1;
    let world_zfar = ((render_distance as u64 + 1 + buffer) * Chunk::DIM as u64) as f32 * SQRT_2;
    let cloud_zfar = 1024.0;
    world_zfar.max(cloud_zfar)
});
