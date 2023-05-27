use super::Renderer;
use bytemuck::Pod;
use std::{marker::PhantomData, mem, slice};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

pub struct Uniform<T> {
    buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    phantom: PhantomData<T>,
}

impl<T: Pod> Uniform<T> {
    pub fn new(
        Renderer { device, .. }: &Renderer,
        data: Option<&T>,
        visibility: wgpu::ShaderStages,
    ) -> Self {
        let buffer = if let Some(data) = data {
            device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(slice::from_ref(data)),
                usage: Self::usage(),
            })
        } else {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: mem::size_of::<T>() as u64,
                usage: Self::usage(),
                mapped_at_creation: false,
            })
        };
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility,
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
            phantom: PhantomData,
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn write(&self, Renderer { queue, .. }: &Renderer, data: &T) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(slice::from_ref(data)));
    }

    fn usage() -> wgpu::BufferUsages {
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
    }
}
