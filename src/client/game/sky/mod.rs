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
use nalgebra::Vector3;
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
    horizon_color: Float3,
    glow_color: Rgb<f32>,
    glow_opacity: f32,
    arc_angle: f32,
    sun_intensity: f32,
    padding: [f32; 2],
    light_intensity: Float3,
}

impl SkyUniformData {
    fn new(time: Time) -> Self {
        let progress = time.stage().progress();
        let config = &CLIENT_CONFIG.sky;
        let sun_dir = time.sky_rotation() * Vector3::x();
        let color = utils::lerp(config.day.color, config.night.color, progress);
        let horizon_color = utils::lerp(
            config.day.horizon_color,
            config.night.horizon_color,
            progress,
        );
        let glow_color = utils::lerp(config.day.glow_color, config.night.glow_color, progress);
        let glow_opacity = Self::glow_opacity(progress);
        let arc_angle = Self::arc_angle(config.day.arc_angle, config.night.arc_angle, progress);
        let sun_intensity = utils::lerp(config.sun_intensity, 1.0, progress);
        let light_intensity = utils::lerp(
            config.day.light_intensity,
            config.night.light_intensity,
            progress,
        );
        Self {
            sun_dir: sun_dir.into(),
            color: color.into(),
            horizon_color: horizon_color.into(),
            glow_color,
            glow_opacity,
            arc_angle,
            sun_intensity,
            padding: Default::default(),
            light_intensity: light_intensity.into(),
        }
    }

    fn glow_opacity(progress: f32) -> f32 {
        1.0 - (progress * 2.0 - 1.0).powi(2)
    }

    fn arc_angle(day: f32, night: f32, progress: f32) -> f32 {
        let t = 1.0 - (1.0 - (progress * 3.0 - 1.0).max(0.0)).abs();
        utils::lerp(day, night, t)
    }
}

#[derive(Deserialize)]
pub struct SkyConfig {
    sun_intensity: f32,
    day: StageConfig,
    night: StageConfig,
    star: StarConfig,
    object: ObjectConfig,
}

#[derive(Deserialize)]
struct StageConfig {
    color: Rgb<f32>,
    horizon_color: Rgb<f32>,
    glow_color: Rgb<f32>,
    arc_angle: f32,
    light_intensity: Rgb<f32>,
}
