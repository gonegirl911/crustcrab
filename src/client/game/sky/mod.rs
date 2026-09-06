pub mod atmosphere;
pub mod object;
pub mod star;

use crate::{
    client::{
        CLIENT_CONFIG,
        event_loop::{Event, EventHandler},
        renderer::{Renderer, Surface, buffer::MemoryState, uniform::Uniform},
    },
    server::{ServerEvent, game::clock::Time},
    shared::{
        color::{Float3, Rgb},
        utils,
    },
};
use atmosphere::Atmosphere;
use bytemuck::{Pod, Zeroable};
use object::{ObjectConfig, ObjectSet};
use serde::Deserialize;
use star::{StarConfig, StarDome};
use winit::event::WindowEvent;

pub struct Sky {
    atmosphere: Atmosphere,
    stars: StarDome,
    objects: ObjectSet,
    uniform: Uniform<SkyUniformData>,
    updated_time: Option<Time>,
}

impl Sky {
    pub fn new(
        renderer: &Renderer,
        surface: &Surface,
        player_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let uniform = Uniform::new(
            renderer,
            MemoryState::UNINIT,
            wgpu::ShaderStages::VERTEX_FRAGMENT,
        );
        let atmosphere = Atmosphere::new(
            renderer,
            player_bind_group_layout,
            uniform.bind_group_layout(),
        );
        let stars = StarDome::new(renderer, player_bind_group_layout);
        let objects = ObjectSet::new(
            renderer,
            surface,
            player_bind_group_layout,
            uniform.bind_group_layout(),
        );
        Self {
            atmosphere,
            stars,
            objects,
            uniform,
            updated_time: Some(Default::default()),
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.uniform.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.uniform.bind_group()
    }

    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_bind_group: &wgpu::BindGroup,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(Default::default()),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        self.atmosphere.draw(
            &mut render_pass,
            player_bind_group,
            self.uniform.bind_group(),
        );
        self.stars.draw(&mut render_pass, player_bind_group);
        self.objects.draw(
            &mut render_pass,
            player_bind_group,
            self.uniform.bind_group(),
        );
    }
}

impl EventHandler for Sky {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.stars.handle(event, renderer);
        self.objects.handle(event, ());

        match *event {
            Event::ServerEvent(ServerEvent::TimeUpdated(time)) => {
                self.updated_time = Some(time);
            }
            Event::WindowEvent(WindowEvent::RedrawRequested) => {
                if let Some(time) = self.updated_time.take() {
                    self.uniform.set(renderer, &SkyUniformData::new(time));
                }
            }
            _ => {}
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct SkyUniformData {
    sun_dir: Float3,
    color: Float3,
    horizon_color: Rgb<f32>,
    glow_opacity: f32,
    glow_color: Rgb<f32>,
    arc_angle: f32,
    sunlight_intensity: Float3,
}

impl SkyUniformData {
    fn new(time: Time) -> Self {
        let config = &CLIENT_CONFIG.sky;
        let nightness = time.nightness();
        Self {
            sun_dir: time.sun_dir().into(),
            color: config.color(nightness).into(),
            horizon_color: config.horizon_color(nightness),
            glow_opacity: Self::glow_opacity(nightness),
            glow_color: config.glow_color(nightness),
            arc_angle: config.arc_angle(nightness),
            sunlight_intensity: config.sunlight_intensity(nightness).into(),
        }
    }

    fn glow_opacity(nightness: f32) -> f32 {
        1.0 - (nightness * 2.0 - 1.0).powi(2)
    }
}

#[derive(Deserialize)]
pub struct SkyConfig {
    day: TimePhaseConfig,
    night: TimePhaseConfig,
    star: StarConfig,
    object: ObjectConfig,
}

impl SkyConfig {
    fn color(&self, nightness: f32) -> Rgb<f32> {
        utils::lerp(self.day.color, self.night.color, nightness)
    }

    fn horizon_color(&self, nightness: f32) -> Rgb<f32> {
        utils::lerp(self.day.horizon_color, self.night.horizon_color, nightness)
    }

    fn glow_color(&self, nightness: f32) -> Rgb<f32> {
        utils::lerp(self.day.glow_color, self.night.glow_color, nightness)
    }

    fn arc_angle(&self, nightness: f32) -> f32 {
        let t = 1.0 - (1.0 - (nightness * 3.0 - 1.0).max(0.0)).abs();
        utils::lerp(self.day.arc_angle, self.night.arc_angle, t)
    }

    pub fn sunlight_intensity(&self, nightness: f32) -> Rgb<f32> {
        utils::lerp(
            self.day.sunlight_intensity,
            self.night.sunlight_intensity,
            nightness,
        )
    }
}

#[derive(Deserialize)]
struct TimePhaseConfig {
    color: Rgb<f32>,
    horizon_color: Rgb<f32>,
    glow_color: Rgb<f32>,
    arc_angle: f32,
    sunlight_intensity: Rgb<f32>,
}
