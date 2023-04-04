use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{Renderer, Uniform},
    },
    color::{Float3, Rgb},
    server::{
        game::clock::{Stage, TimeData},
        ServerEvent,
    },
};
use bytemuck::{Pod, Zeroable};

pub struct Sky {
    uniform: Uniform<SkyUniformData>,
    updated_data: Option<TimeData>,
}

impl Sky {
    const DAY_INTENSITY: Rgb<f32> = Rgb::splat(1.0);
    const NIGHT_INTENSITY: Rgb<f32> = Rgb::new(0.15, 0.15, 0.3);

    pub fn new(renderer: &Renderer) -> Self {
        Self {
            uniform: Uniform::new(renderer, wgpu::ShaderStages::VERTEX),
            updated_data: Some(Default::default()),
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.uniform.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.uniform.bind_group()
    }

    fn light_intensity(stage: Stage) -> Rgb<f32> {
        match stage {
            Stage::Dawn { progress } => Self::NIGHT_INTENSITY.lerp(Self::DAY_INTENSITY, progress),
            Stage::Day => Self::DAY_INTENSITY,
            Stage::Dusk { progress } => Self::DAY_INTENSITY.lerp(Self::NIGHT_INTENSITY, progress),
            Stage::Night => Self::NIGHT_INTENSITY,
        }
    }
}

impl EventHandler for Sky {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        match event {
            Event::UserEvent(ServerEvent::TimeUpdated(data)) => {
                self.updated_data = Some(*data);
            }
            Event::RedrawRequested(_) => {
                if let Some(data) = self.updated_data {
                    self.uniform.write(
                        renderer,
                        &SkyUniformData::new(Self::light_intensity(data.stage())),
                    );
                }
            }
            Event::RedrawEventsCleared => {
                self.updated_data = None;
            }
            _ => {}
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct SkyUniformData {
    light_intensity: Float3,
}

impl SkyUniformData {
    fn new(light_intensity: Rgb<f32>) -> Self {
        Self {
            light_intensity: light_intensity.into(),
        }
    }
}
