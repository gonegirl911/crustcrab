use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{Renderer, Uniform},
    },
    server::ServerEvent,
};
use bytemuck::{Pod, Zeroable};

pub struct Clock {
    uniform: Uniform<ClockUniformData>,
    updated_time: Option<f32>,
}

impl Clock {
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

impl EventHandler for Clock {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        match event {
            Event::UserEvent(ServerEvent::TimeUpdated { time }) => {
                self.updated_time = Some(*time);
            }
            Event::RedrawRequested(_) => {
                if let Some(time) = self.updated_time {
                    self.uniform.update(renderer, &ClockUniformData::new(time));
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
struct ClockUniformData {
    time: f32,
}

impl ClockUniformData {
    fn new(time: f32) -> Self {
        Self { time }
    }
}
