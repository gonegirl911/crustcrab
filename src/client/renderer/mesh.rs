use super::Renderer;
use bytemuck::Pod;
use std::{marker::PhantomData, mem};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

pub struct Mesh<V> {
    vertex_buffer: wgpu::Buffer,
    phantom: PhantomData<V>,
}

impl<V: Vertex> Mesh<V> {
    pub fn new(Renderer { device, .. }: &Renderer, vertices: &[V]) -> Self {
        Self {
            vertex_buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            phantom: PhantomData,
        }
    }

    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..self.len(), 0..1);
    }

    fn len(&self) -> u32 {
        (self.vertex_buffer.size() / mem::size_of::<V>() as u64) as u32
    }
}

pub struct IndexedMesh<V, I> {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    phantom: PhantomData<(V, I)>,
}

impl<V: Vertex, I: Index> IndexedMesh<V, I> {
    pub fn new(Renderer { device, .. }: &Renderer, vertices: &[V], indices: &[I]) -> Self {
        Self {
            vertex_buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            index_buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            phantom: PhantomData,
        }
    }

    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), I::FORMAT);
        render_pass.draw_indexed(0..self.len(), 0, 0..1);
    }

    fn len(&self) -> u32 {
        (self.index_buffer.size() / mem::size_of::<I>() as u64) as u32
    }
}

pub trait Vertex: Pod {
    const ATTRIBS: &'static [wgpu::VertexAttribute];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: Self::ATTRIBS,
        }
    }
}

pub trait Index: Pod {
    const FORMAT: wgpu::IndexFormat;
}

impl Index for u16 {
    const FORMAT: wgpu::IndexFormat = wgpu::IndexFormat::Uint16;
}

impl Index for u32 {
    const FORMAT: wgpu::IndexFormat = wgpu::IndexFormat::Uint32;
}
