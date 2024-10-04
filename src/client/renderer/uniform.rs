use super::{
    Renderer,
    buffer::{MemoryState, UniformBuffer},
};
use bytemuck::Pod;

pub struct Uniform<T> {
    buffer: UniformBuffer<T>,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl<T: Pod> Uniform<T> {
    pub fn new(
        renderer @ Renderer { device, .. }: &Renderer,
        state: MemoryState<T>,
        visibility: wgpu::ShaderStages,
    ) -> Self {
        let buffer = UniformBuffer::new(renderer, state);
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
}
