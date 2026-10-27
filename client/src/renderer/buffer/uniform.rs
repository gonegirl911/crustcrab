use super::{Buffer, MemoryState};
use crate::renderer::Renderer;
use bytemuck::Pod;
use std::ops::Deref;

pub(in crate::renderer) struct UniformBuffer<T>(Buffer<T>);

impl<T: Pod> UniformBuffer<T> {
    pub fn new(renderer: &Renderer, state: MemoryState<T>) -> Self {
        Self(Buffer::<T>::new(
            renderer,
            state,
            wgpu::BufferUsages::UNIFORM,
        ))
    }
}

impl<T> Deref for UniformBuffer<T> {
    type Target = Buffer<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
