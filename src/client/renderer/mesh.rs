use super::{buffer::Buffer, Renderer};
use bytemuck::Pod;
use std::{cmp::Reverse, mem};

pub struct Mesh<V> {
    vertex_buffer: Buffer<[V]>,
}

impl<V: Pod> Mesh<V> {
    pub fn from_data(renderer: &Renderer, vertices: &[V]) -> Self {
        Self {
            vertex_buffer: Buffer::<[_]>::new(renderer, Ok(vertices), wgpu::BufferUsages::VERTEX),
        }
    }

    fn uninit_mut(renderer: &Renderer, len: usize) -> Self {
        Self {
            vertex_buffer: Buffer::<[_]>::new(
                renderer,
                Err(len),
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            ),
        }
    }

    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..self.vertex_buffer.len(), 0..1);
    }

    fn write(&self, renderer: &Renderer, vertices: &[V]) {
        self.vertex_buffer.write(renderer, vertices);
    }
}

pub struct TransparentMesh<C, V> {
    vertices: Vec<(C, [V; 3])>,
    mesh: Mesh<V>,
}

impl<C, V: Pod> TransparentMesh<C, V> {
    pub fn from_data<F>(renderer: &Renderer, vertices: &[V], mut coords: F) -> Self
    where
        F: FnMut([V; 3]) -> C,
    {
        Self {
            mesh: Mesh::uninit_mut(renderer, vertices.len()),
            vertices: vertices
                .chunks_exact(3)
                .map(|v| {
                    let v = v.try_into().unwrap_or_else(|_| unreachable!());
                    (coords(v), v)
                })
                .collect(),
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
        self.mesh.write(
            renderer,
            &self
                .vertices
                .iter()
                .flat_map(|(_, v)| v)
                .copied()
                .collect::<Vec<_>>(),
        );
        self.mesh.draw(render_pass);
    }
}

pub struct IndexedMesh<V, I> {
    vertex_buffer: Buffer<[V]>,
    index_buffer: Buffer<[I]>,
}

impl<V: Pod, I: Index> IndexedMesh<V, I> {
    pub fn from_data(renderer: &Renderer, vertices: &[V], indices: &[I]) -> Self {
        Self {
            vertex_buffer: Buffer::<[_]>::new(renderer, Ok(vertices), wgpu::BufferUsages::VERTEX),
            index_buffer: Buffer::<[_]>::new(renderer, Ok(indices), wgpu::BufferUsages::INDEX),
        }
    }

    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), I::FORMAT);
        render_pass.draw_indexed(0..self.index_buffer.len(), 0, 0..1);
    }
}

pub trait Vertex: Pod {
    const ATTRIBS: &'static [wgpu::VertexAttribute];
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Vertex;

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: Self::STEP_MODE,
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
