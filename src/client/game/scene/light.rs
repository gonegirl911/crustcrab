use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{Renderer, Uniform},
    },
    server::ServerEvent,
};
use bytemuck::{Pod, Zeroable};

pub struct Skylight {
    uniform: Uniform<SkylightUniformData>,
    updated_time: Option<f32>,
}

impl Skylight {
    pub fn new(renderer: &Renderer) -> Self {
        Self {
            uniform: Uniform::new(renderer),
            updated_time: Some(0.0),
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.uniform.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.uniform.bind_group()
    }
}

impl EventHandler for Skylight {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        match event {
            Event::UserEvent(ServerEvent::TimeUpdated { time }) => {
                self.updated_time = Some(*time);
            }
            Event::RedrawRequested(_) => {
                if let Some(time) = self.updated_time {
                    self.uniform
                        .update(renderer, &SkylightUniformData::new(todo!()));
                }
            }
            Event::RedrawEventsCleared => {
                self.updated_time = None;
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
