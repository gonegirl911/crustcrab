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
    pub fn from_value_mut(renderer: &Renderer, value: &T, visibility: wgpu::ShaderStages) -> Self {
        Self::new(renderer, Some(value), visibility, true)
    }

    pub fn uninit_mut(renderer: &Renderer, visibility: wgpu::ShaderStages) -> Self {
        Self::new(renderer, None, visibility, true)
    }

    fn new(
        Renderer { device, .. }: &Renderer,
        value: Option<&T>,
        visibility: wgpu::ShaderStages,
        is_mutable: bool,
    ) -> Self {
        let buffer = if let Some(value) = value {
            device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(slice::from_ref(value)),
                usage: Self::usage(is_mutable),
            })
        } else {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: mem::size_of::<T>() as u64,
                usage: Self::usage(is_mutable),
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

    pub fn set(&self, Renderer { queue, .. }: &Renderer, value: &T) {
        queue.write_buffer(
            &self.buffer,
            0,
            bytemuck::cast_slice(slice::from_ref(value)),
        );
    }

    fn usage(is_mutable: bool) -> wgpu::BufferUsages {
        if is_mutable {
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
        } else {
            wgpu::BufferUsages::UNIFORM
        }
    }
}
