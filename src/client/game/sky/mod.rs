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
    shared::color::{Float3, Rgb},
};
use bytemuck::{Pod, Zeroable};
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
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(Default::default()),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
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

        match event {
            Event::UserEvent(ServerEvent::TimeUpdated(time)) => {
                self.updated_time = Some(*time);
            }
            Event::MainEventsCleared => {
                if let Some(time) = self.updated_time.take() {
                    self.uniform
                        .set(renderer, &CLIENT_CONFIG.sky.sky_data(time.stage()));
                }
            }
            _ => {}
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct SkyUniformData {
    light_intensity: Rgb<f32>,
    sun_intensity: f32,
    color: Float3,
    horizon_color: Float3,
}

impl SkyUniformData {
    fn new(
        light_intensity: Rgb<f32>,
        sun_intensity: f32,
        color: Rgb<f32>,
        horizon_color: Rgb<f32>,
    ) -> Self {
        Self {
            light_intensity,
            sun_intensity,
            color: color.into(),
            horizon_color: horizon_color.into(),
        }
    }
}

#[derive(Deserialize)]
pub struct SkyConfig {
    sun_intensity: f32,
    day: PeriodConfig,
    night: PeriodConfig,
    object: ObjectConfig,
    star: StarConfig,
}

#[derive(Deserialize)]
struct PeriodConfig {
    intensity: Rgb<f32>,
    color: Rgb<f32>,
    horizon_color: Rgb<f32>,
}

impl SkyConfig {
    fn sky_data(&self, stage: Stage) -> SkyUniformData {
        SkyUniformData::new(
            stage.lerp(self.day.intensity, self.night.intensity),
            stage.lerp(self.sun_intensity, 0.0),
            stage.lerp(self.day.color, self.night.color),
            stage.lerp(self.day.horizon_color, self.night.horizon_color),
        )
    }
}
