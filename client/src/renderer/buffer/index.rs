use super::{Buffer, MemoryState};
use crate::renderer::Renderer;
use bytemuck::Pod;
use std::ops::Deref;

pub(crate) struct IndexBuffer<I>(Buffer<[I]>);

impl<I: Pod> IndexBuffer<I> {
    pub(crate) fn new(renderer: &Renderer, state: MemoryState<[I], usize>) -> Self {
        Self(Buffer::<[_]>::new(
            renderer,
            state,
            wgpu::BufferUsages::INDEX,
        ))
    }
}

impl<I> Deref for IndexBuffer<I> {
    type Target = Buffer<[I]>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub(crate) impl(self) trait Index: Pod {
    const FORMAT: wgpu::IndexFormat;
}

impl Index for u16 {
    const FORMAT: wgpu::IndexFormat = wgpu::IndexFormat::Uint16;
}

impl Index for u32 {
    const FORMAT: wgpu::IndexFormat = wgpu::IndexFormat::Uint32;
}
