use super::{Renderer, buffer::VertexBuffer};
use crate::client::renderer::buffer::MemoryState;
use bytemuck::Pod;
use image::RgbaImage;
use std::{
    cmp::{Ordering, Reverse},
    fs,
    path::Path,
    slice,
};

pub fn load_rgba<P: AsRef<Path>>(path: P) -> RgbaImage {
    let path = path.as_ref();
    image::open(path)
        .unwrap_or_else(|e| panic!("failed to open {}: {e}", path.display()))
        .into_rgba8()
}

// ------------------------------------------------------------------------------------------------

pub fn read_wgsl<P: AsRef<Path>>(path: P) -> wgpu::ShaderModuleDescriptor<'static> {
    let path = path.as_ref();
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to open {}: {e}", path.display()));
    wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(contents.into()),
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

pub struct TransparentMesh<C, V> {
    faces: Vec<(C, [V; 6])>,
    vertices: Vec<V>,
    buffer: VertexBuffer<V>,
}

impl<C, V: Pod> TransparentMesh<C, V> {
    pub fn try_new<F>(renderer: &Renderer, vertices: &[V], mut coords: F) -> Option<Self>
    where
        F: FnMut(&[V]) -> C,
    {
        let (faces, []) = vertices.as_chunks() else {
            unreachable!();
        };
        Some(Self {
            buffer: VertexBuffer::try_new(renderer, MemoryState::Uninit(vertices.len()))?,
            faces: faces.iter().map(|v| (coords(v), *v)).collect(),
            vertices: Vec::with_capacity(vertices.len()),
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
        self.faces.sort_unstable_by_key(|(c, _)| Reverse(dist(c)));
        self.vertices.clear();
        self.vertices.extend(self.faces.iter().flat_map(|&(_, v)| v));
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
