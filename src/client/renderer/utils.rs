use super::{Renderer, buffer::VertexBuffer};
use crate::client::renderer::buffer::MemoryState;
use bytemuck::Pod;
use std::cmp::{Ordering, Reverse};

pub struct TransparentMesh<C, V> {
    data: Vec<(C, [V; 6])>,
    vertices: Vec<V>,
    buffer: VertexBuffer<V>,
}

impl<C, V: Pod> TransparentMesh<C, V> {
    pub fn try_new<F>(renderer: &Renderer, vertices: &[V], mut coords: F) -> Option<Self>
    where
        F: FnMut(&[V]) -> C,
    {
        assert_eq!(vertices.len() % 6, 0);
        Some(Self {
            buffer: VertexBuffer::try_new(renderer, MemoryState::Uninit(vertices.len()))?,
            data: vertices.array_chunks().map(|v| (coords(v), *v)).collect(),
            vertices: Vec::with_capacity(vertices.len()),
        })
    }

    pub fn draw<D, F>(
        &mut self,
        renderer: &Renderer,
        render_pass: &mut wgpu::RenderPass,
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

pub struct TotalOrd(pub f32);

impl PartialEq for TotalOrd {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for TotalOrd {}

impl PartialOrd for TotalOrd {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TotalOrd {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}
