use super::{
    Buffer, MemoryState,
    index::{Index, IndexBuffer},
};
use crate::renderer::Renderer;
use bytemuck::Pod;
use std::ops::Deref;

pub(crate) struct VertexBuffer<V>(Buffer<[V]>);

impl<V> VertexBuffer<V> {
    pub(crate) fn draw_indexed<I: Index>(
        &self,
        render_pass: &mut wgpu::RenderPass,
        index_buffer: &IndexBuffer<I>,
    ) {
        render_pass.set_vertex_buffer(0, self.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), I::FORMAT);
        render_pass.draw_indexed(0..index_buffer.len(), 0, 0..1);
    }

    pub(crate) fn draw_indexed_instanced<I: Index, E>(
        &self,
        render_pass: &mut wgpu::RenderPass,
        index_buffer: &IndexBuffer<I>,
        instance_buffer: &VertexBuffer<E>,
    ) {
        render_pass.set_vertex_buffer(0, self.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), I::FORMAT);
        render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
        render_pass.draw_indexed(0..index_buffer.len(), 0, 0..instance_buffer.len());
    }
}

impl<V: Pod> VertexBuffer<V> {
    pub(crate) fn new(renderer: &Renderer, state: MemoryState<[V], usize>) -> Self {
        Self(Buffer::<[_]>::new(
            renderer,
            state,
            wgpu::BufferUsages::VERTEX,
        ))
    }
}

impl<V> Deref for VertexBuffer<V> {
    type Target = Buffer<[V]>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
