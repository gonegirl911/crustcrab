pub(crate) mod index;
pub(super) mod uniform;
pub(crate) mod vertex;

use super::Renderer;
use bytemuck::Pod;
use std::{marker::PhantomData, ops::Deref, slice};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

pub(crate) struct Buffer<T: ?Sized> {
    buffer: wgpu::Buffer,
    phantom: PhantomData<T>,
}

impl<T: Pod> Buffer<T> {
    pub(super) fn new(
        renderer: &Renderer,
        state: MemoryState<T>,
        usage: wgpu::BufferUsages,
    ) -> Self {
        Self {
            buffer: Buffer::<[_]>::new(renderer, MemoryState::from_ref(state), usage).buffer,
            phantom: PhantomData,
        }
    }

    pub(super) fn set(&self, Renderer { queue, .. }: &Renderer, value: &T) {
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(value));
    }
}

impl<T> Buffer<[T]> {
    pub(crate) fn len(&self) -> u32 {
        (self.buffer.size() / size_of::<T>() as u64) as u32
    }
}

impl<T: Pod> Buffer<[T]> {
    fn new(renderer: &Renderer, state: MemoryState<[T], usize>, usage: wgpu::BufferUsages) -> Self {
        Self::try_new(renderer, state, usage).unwrap()
    }

    fn try_new(
        Renderer { device, .. }: &Renderer,
        state: MemoryState<[T], usize>,
        usage: wgpu::BufferUsages,
    ) -> Option<Self> {
        const { assert!(size_of::<T>() != 0) };

        let len = state.len();
        if len == 0 {
            return None;
        }

        let size = len * size_of::<T>();
        assert!(size.is_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT as usize));

        Some(Self {
            buffer: match state {
                MemoryState::Immutable(data) => device.create_buffer_init(&BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(data),
                    usage,
                }),
                MemoryState::Uninit(_) => device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: size as u64,
                    usage: usage | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
            },
            phantom: PhantomData,
        })
    }
}

impl<T: ?Sized> Deref for Buffer<T> {
    type Target = wgpu::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

pub(crate) enum MemoryState<'a, T: ?Sized, U = ()> {
    Immutable(&'a T),
    Uninit(U),
}

impl<T: ?Sized> MemoryState<'_, T, ()> {
    pub(crate) const UNINIT: Self = Self::Uninit(());
}

impl<'a, T> MemoryState<'a, [T], usize> {
    fn from_ref(state: MemoryState<'a, T>) -> Self {
        match state {
            MemoryState::Immutable(value) => Self::Immutable(slice::from_ref(value)),
            MemoryState::Uninit(()) => Self::Uninit(1),
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::Immutable(data) => data.len(),
            &Self::Uninit(len) => len,
        }
    }
}

impl<T: ?Sized, U: Clone> Clone for MemoryState<'_, T, U> {
    fn clone(&self) -> Self {
        match self {
            &Self::Immutable(data) => Self::Immutable(data),
            Self::Uninit(fallback) => Self::Uninit(fallback.clone()),
        }
    }
}

impl<T: ?Sized, U: Copy> Copy for MemoryState<'_, T, U> {}
