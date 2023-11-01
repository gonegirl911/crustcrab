use super::{
    buffer::{MemoryState, VertexBuffer},
    Renderer,
};
use bytemuck::Pod;
use std::cmp::Reverse;

pub struct TransparentMesh<C, V> {
    data: Vec<(C, [V; 6])>,
    vertices: Vec<V>,
    buffer: VertexBuffer<V>,
}

impl<C, V: Pod> TransparentMesh<C, V> {
    pub fn new<F>(renderer: &Renderer, vertices: &[V], mut coords: F) -> Self
    where
        F: FnMut(&[V]) -> C,
    {
        Self {
            data: vertices
                .chunks_exact(6)
                .map(|v| (coords(v), v.try_into().unwrap_or_else(|_| unreachable!())))
                .collect(),
            vertices: Vec::with_capacity(vertices.len()),
            buffer: VertexBuffer::new(renderer, MemoryState::Uninit(vertices.len())),
        }
    }

    pub fn draw<'a, D, F>(
        &'a mut self,
        renderer: &Renderer,
        render_pass: &mut wgpu::RenderPass<'a>,
        mut dist: F,
    ) where
        D: Ord,
        F: FnMut(&C) -> D,
    {
        self.data.sort_unstable_by_key(|(c, _)| Reverse(dist(c)));
        self.vertices.clear();
        self.vertices.extend(self.data.iter().flat_map(|&(_, v)| v));
        self.buffer.write(renderer, &self.vertices);
        self.buffer.draw(render_pass);
    }
}
