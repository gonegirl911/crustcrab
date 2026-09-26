use super::{
    Renderer,
    buffer::{IndexBuffer, VertexBuffer},
};
use crate::client::renderer::buffer::MemoryState;
use bytemuck::Pod;
use image::RgbaImage;
use std::{
    cmp::{Ordering, Reverse},
    fs,
    path::Path,
    slice,
};

pub fn read_wgsl<P: AsRef<Path>>(path: P) -> wgpu::ShaderModuleDescriptor<'static> {
    let path = path.as_ref();
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(contents.into()),
    }
}

// ------------------------------------------------------------------------------------------------

pub trait Vertex: Pod {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Vertex;
    const ATTRIBS: &[wgpu::VertexAttribute];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: Self::STEP_MODE,
            attributes: Self::ATTRIBS,
        }
    }
}

// ------------------------------------------------------------------------------------------------

pub trait Immediates: Pod {
    const SIZE: u32 = {
        let size = size_of::<Self>();
        assert!(usize::BITS <= u32::BITS || size <= u32::MAX as usize);
        size as u32
    };

    fn set(&self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_immediates(0, bytemuck::cast_slice(slice::from_ref(self)));
    }
}

// ------------------------------------------------------------------------------------------------

pub struct BlendedMesh<C, V> {
    face_indices: Vec<(C, u32)>,
    vertex_buffer: VertexBuffer<V>,
    index_buffer: IndexBuffer<u32>,
    indices: Vec<u32>,
}

impl<C, V: Pod> BlendedMesh<C, V> {
    pub fn try_new<F>(renderer: &Renderer, vertices: &[V], mut coords: F) -> Option<Self>
    where
        F: FnMut(&[V]) -> C,
    {
        let (faces, []) = vertices.as_chunks::<6>() else {
            unreachable!();
        };
        let vertex_buffer = VertexBuffer::try_new(renderer, MemoryState::Immutable(vertices))?;
        Some(Self {
            face_indices: faces
                .iter()
                .enumerate()
                .map(|(i, v)| (coords(v), i as u32 * 6))
                .collect(),
            vertex_buffer,
            index_buffer: IndexBuffer::new(renderer, MemoryState::Uninit(vertices.len())),
            indices: Vec::with_capacity(vertices.len()),
        })
    }

    #[rustfmt::skip]
    pub fn draw<D, F>(
        &mut self,
        renderer: &Renderer,
        render_pass: &mut wgpu::RenderPass,
        mut dist: F,
    ) where
        D: Ord,
        F: FnMut(&C) -> D,
    {
        self.face_indices.sort_unstable_by_key(|(c, _)| Reverse(dist(c)));
        self.indices.clear();
        self.indices.extend(self.face_indices.iter().flat_map(|&(_, base)| base..base + 6));
        self.index_buffer.write(renderer, &self.indices);
        self.vertex_buffer.draw_indexed(render_pass, &self.index_buffer);
    }
}

pub struct TotalOrd(pub f64);

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

// ------------------------------------------------------------------------------------------------

pub fn load_rgba<P: AsRef<Path>>(path: P) -> RgbaImage {
    let path = path.as_ref();
    image::open(path)
        .unwrap_or_else(|e| panic!("failed to open {}: {e}", path.display()))
        .into_rgba8()
}
