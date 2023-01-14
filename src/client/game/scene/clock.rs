use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::Renderer,
    },
    server::ServerEvent,
};
use bytemuck::{Pod, Zeroable};
use std::{mem, slice};

pub struct Clock {
    uniform: ClockUniform,
    updated_time: Option<f32>,
}

impl Clock {
    pub fn new(renderer: &Renderer) -> Self {
        Self {
            uniform: ClockUniform::new(renderer),
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
                    self.uniform.update(renderer, &ClockUniformData { time });
                }
            }
            Event::RedrawEventsCleared => {
                self.updated_time = None;
            }
            _ => {}
        }
    }
}

struct ClockUniform {
    buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl ClockUniform {
    fn new(Renderer { device, .. }: &Renderer) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: mem::size_of::<ClockUniformData>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        Self {
            buffer,
            bind_group_layout,
            bind_group,
        }
    }

    fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    fn update(&self, Renderer { queue, .. }: &Renderer, data: &ClockUniformData) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(slice::from_ref(data)))
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct ClockUniformData {
    time: f32,
}
