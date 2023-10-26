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
    server::{game::clock::Stage, ServerEvent},
    shared::color::{Float3, Rgb},
};
use bytemuck::{Pod, Zeroable};
use serde::Deserialize;

pub struct Sky {
    atmosphere: Atmosphere,
    stars: StarDome,
    objects: ObjectSet,
    uniform: Uniform<SkyUniformData>,
    updated_stage: Result<Stage, Stage>,
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
            updated_stage: Ok(Default::default()),
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

        match event {
            Event::UserEvent(ServerEvent::TimeUpdated(time)) => {
                let curr = time.stage();
                if self.updated_stage.is_ok() || self.updated_stage.is_err_and(|prev| prev != curr)
                {
                    self.updated_stage = Ok(curr);
                }
            }
            Event::MainEventsCleared => {
                if let Ok(stage) = self.updated_stage {
                    self.uniform.set(renderer, &CLIENT_CONFIG.sky.data(stage));
                    self.updated_stage = Err(stage);
                }
            }
            _ => {}
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct SkyUniformData {
    color: Float3,
    horizon_color: Rgb<f32>,
    sun_intensity: f32,
    light_intensity: Float3,
}

impl SkyUniformData {
    fn new(
        color: Rgb<f32>,
        horizon_color: Rgb<f32>,
        sun_intensity: f32,
        light_intensity: Rgb<f32>,
    ) -> Self {
        Self {
            color: color.into(),
            horizon_color,
            sun_intensity,
            light_intensity: light_intensity.into(),
        }
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
    fn data(&self, stage: Stage) -> SkyUniformData {
        SkyUniformData::new(
            stage.lerp(self.day.color, self.night.color),
            stage.lerp(self.day.horizon_color, self.night.horizon_color),
            stage.lerp(self.sun_intensity, 1.0),
            stage.lerp(self.day.light_intensity, self.night.light_intensity),
        )
    }
}

#[derive(Deserialize)]
struct StageConfig {
    color: Rgb<f32>,
    horizon_color: Rgb<f32>,
    light_intensity: Rgb<f32>,
}
