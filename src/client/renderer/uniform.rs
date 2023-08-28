use super::{buffer::Buffer, Renderer};
use bytemuck::Pod;

pub struct Uniform<T> {
    buffer: Buffer<T>,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl<T: Pod> Uniform<T> {
    pub fn from_value_mut(renderer: &Renderer, value: &T, visibility: wgpu::ShaderStages) -> Self {
        Self::new(renderer, Some(value), visibility, true)
    }

    pub fn uninit_mut(renderer: &Renderer, visibility: wgpu::ShaderStages) -> Self {
        Self::new(renderer, None, visibility, true)
    }

    fn new(
        renderer @ Renderer { device, .. }: &Renderer,
        value: Option<&T>,
        visibility: wgpu::ShaderStages,
        is_mutable: bool,
    ) -> Self {
        let buffer = if let Some(value) = value {
            Buffer::<T>::new(renderer, Some(value), Self::usage(is_mutable))
        } else {
            Buffer::<T>::new(renderer, None, Self::usage(is_mutable))
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
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn set(&self, renderer: &Renderer, value: &T) {
        self.buffer.set(renderer, value);
    }

    fn usage(is_mutable: bool) -> wgpu::BufferUsages {
        if is_mutable {
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
        } else {
            wgpu::BufferUsages::UNIFORM
        }
    }
}
