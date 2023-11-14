mod atmosphere;
mod object;
mod star;

use self::{
    atmosphere::Atmosphere,
    object::{ObjectConfig, ObjectSet},
    star::{StarConfig, StarDome},
};
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{buffer::MemoryState, uniform::Uniform, Renderer},
        CLIENT_CONFIG,
    },
    server::{
        game::clock::{Stage, Time},
        ServerEvent,
    },
    shared::{
        color::{Float3, Rgb},
        utils,
    },
};
use bytemuck::{Pod, Zeroable};
use nalgebra::Vector3;
use serde::Deserialize;

pub struct Sky {
    atmosphere: Atmosphere,
    stars: StarDome,
    objects: ObjectSet,
    uniform: Uniform<SkyUniformData>,
    updated_time: Option<Time>,
}

impl Sky {
    pub fn new(renderer: &Renderer, player_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
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
            Event::UserEvent(ServerEvent::TimeUpdated(time)) => {
                self.updated_time = Some(time);
            }
            Event::MainEventsCleared => {
                if let Some(time) = self.updated_time.take() {
                    self.uniform.set(renderer, &CLIENT_CONFIG.sky.data(time));
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
    glow: Glow,
    sun_intensity: f32,
    padding: [f32; 2],
    light_intensity: Float3,
}

impl SkyUniformData {
    fn new(
        sun_dir: Vector3<f32>,
        color: Rgb<f32>,
        horizon_color: Rgb<f32>,
        glow: Glow,
        sun_intensity: f32,
        light_intensity: Rgb<f32>,
    ) -> Self {
        Self {
            sun_dir: sun_dir.into(),
            color: color.into(),
            horizon_color: horizon_color.into(),
            glow,
            sun_intensity,
            padding: Default::default(),
            light_intensity: light_intensity.into(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct Glow {
    color: Rgb<f32>,
    opacity: f32,
    angle: f32,
}

impl Glow {
    fn new(stage: Stage, day: GlowConfig, night: GlowConfig) -> Self {
        Self {
            color: stage.lerp(day.color, night.color),
            opacity: Self::opacity(stage),
            angle: Self::angle(stage, day.angle, night.angle),
        }
    }

    fn opacity(stage: Stage) -> f32 {
        if let Stage::Dawn { progress } | Stage::Dusk { progress } = stage {
            Self::decelerate(1.0 - (progress * 2.0 - 1.0).abs())
        } else {
            0.0
        }
    }

    fn angle(stage: Stage, day: f32, night: f32) -> f32 {
        match stage {
            Stage::Dawn { progress } => -utils::lerp(night, day, Self::progress(progress)),
            Stage::Day => 0.0,
            Stage::Dusk { progress } => utils::lerp(day, night, Self::progress(progress)),
            Stage::Night => 0.0,
        }
    }

    fn progress(progress: f32) -> f32 {
        1.0 - (1.0 - (progress * 3.0 - 1.0).max(0.0)).abs()
    }

    fn decelerate(input: f32) -> f32 {
        1.0 - (1.0 - input).powi(2)
    }
}

#[derive(Deserialize)]
pub struct SkyConfig {
    sun_intensity: f32,
    day: StageConfig,
    night: StageConfig,
    object: ObjectConfig,
    star: StarConfig,
}

impl SkyConfig {
    fn data(&self, time: Time) -> SkyUniformData {
        let stage = time.stage();
        SkyUniformData::new(
            time.sky_rotation() * Vector3::x(),
            stage.lerp(self.day.color, self.night.color),
            stage.lerp(self.day.horizon_color, self.night.horizon_color),
            Glow::new(stage, self.day.glow, self.night.glow),
            stage.lerp(self.sun_intensity, 1.0),
            stage.lerp(self.day.light_intensity, self.night.light_intensity),
        )
    }
}

#[derive(Deserialize)]
struct StageConfig {
    color: Rgb<f32>,
    horizon_color: Rgb<f32>,
    glow: GlowConfig,
    light_intensity: Rgb<f32>,
}

#[derive(Clone, Copy, Deserialize)]
struct GlowConfig {
    color: Rgb<f32>,
    angle: f32,
}
