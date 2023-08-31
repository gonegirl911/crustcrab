mod object;
mod star;

use self::{
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
    shared::{color::Rgb, utils},
};
use bytemuck::{Pod, Zeroable};
use serde::Deserialize;

pub struct Sky {
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
        let stars = StarDome::new(
            renderer,
            player_bind_group_layout,
            uniform.bind_group_layout(),
        );
        let objects = ObjectSet::new(
            renderer,
            player_bind_group_layout,
            uniform.bind_group_layout(),
        );
        Self {
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
        self.stars.draw(
            &mut render_pass,
            player_bind_group,
            self.uniform.bind_group(),
        );
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
}

impl SkyUniformData {
    fn new(light_intensity: Rgb<f32>, sun_intensity: f32) -> Self {
        Self {
            light_intensity,
            sun_intensity,
        }
    }
}

#[derive(Deserialize)]
pub struct SkyConfig {
    day_intensity: Rgb<f32>,
    night_intensity: Rgb<f32>,
    sun_intensity: f32,
    object: ObjectConfig,
    star: StarConfig,
}

impl SkyConfig {
    fn sky_data(&self, stage: Stage) -> SkyUniformData {
        match stage {
            Stage::Dawn { progress } => SkyUniformData::new(
                utils::lerp(self.night_intensity, self.day_intensity, progress),
                utils::lerp(0.0, self.sun_intensity, progress),
            ),
            Stage::Day => SkyUniformData::new(self.day_intensity, self.sun_intensity),
            Stage::Dusk { progress } => SkyUniformData::new(
                utils::lerp(self.day_intensity, self.night_intensity, progress),
                utils::lerp(self.sun_intensity, 0.0, progress),
            ),
            Stage::Night => SkyUniformData::new(self.night_intensity, 0.0),
        }
    }
}
