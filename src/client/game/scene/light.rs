use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{Renderer, Uniform},
    },
    server::{
        game::scene::clock::{Stage, TimeData},
        ServerEvent,
    },
};
use bytemuck::{Pod, Zeroable};
use std::ops::RangeInclusive;

pub struct Skylight {
    uniform: Uniform<SkylightUniformData>,
    updated_data: Option<TimeData>,
}

impl Skylight {
    pub fn new(renderer: &Renderer) -> Self {
        Self {
            uniform: Uniform::new(
                renderer,
                wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ),
            updated_data: Some(Default::default()),
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.uniform.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.uniform.bind_group()
    }

    fn light_intensity(TimeData { stage, .. }: TimeData) -> f32 {
        match stage {
            Stage::Night => 0.2,
            Stage::Dawn { progress } => Self::lerp(0.2..=1.0, progress),
            Stage::Day => 1.0,
            Stage::Dusk { progress } => Self::lerp(1.0..=0.2, progress),
        }
    }

    fn lerp(range: RangeInclusive<f32>, t: f32) -> f32 {
        range.start() * (1.0 - t) + range.end() * t
    }
}

impl EventHandler for Skylight {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        match event {
            Event::UserEvent(ServerEvent::TimeUpdated(data)) => {
                self.updated_data = Some(*data);
            }
            Event::RedrawRequested(_) => {
                if let Some(data) = self.updated_data {
                    self.uniform.update(
                        renderer,
                        &SkylightUniformData::new(Self::light_intensity(data)),
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
struct SkylightUniformData {
    light_intensity: f32,
}

impl SkylightUniformData {
    fn new(light_intensity: f32) -> Self {
        Self { light_intensity }
    }
}
