use super::{
    buffer::{MemoryState, VertexBuffer},
    Renderer,
};
use bytemuck::Pod;
use std::cmp::Reverse;

pub struct TransparentMesh<C, V> {
    vertices: Vec<(C, [V; 3])>,
    buffer: VertexBuffer<V>,
}

impl<C, V: Pod> TransparentMesh<C, V> {
    pub fn new<F>(renderer: &Renderer, vertices: &[V], mut coords: F) -> Self
    where
        F: FnMut([V; 3]) -> C,
    {
        Self {
            vertices: vertices
                .chunks_exact(3)
                .map(|v| {
                    let v = v.try_into().unwrap_or_else(|_| unreachable!());
                    (coords(v), v)
                })
                .collect(),
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
        self.vertices.sort_by_key(|(c, _)| Reverse(dist(c)));
        self.buffer.write(
            renderer,
            &self
                .vertices
                .iter()
                .flat_map(|(_, v)| v)
                .copied()
                .collect::<Vec<_>>(),
        );
        self.buffer.draw(render_pass);
    }
}
