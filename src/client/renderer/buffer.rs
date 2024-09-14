use super::Renderer;
use bytemuck::Pod;
use std::{marker::PhantomData, ops::Deref, slice};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

pub struct VertexBuffer<V>(Buffer<[V]>);

impl<V> VertexBuffer<V> {
    pub fn draw(&self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_vertex_buffer(0, self.slice(..));
        render_pass.draw(0..self.len(), 0..1);
    }

    pub fn draw_indexed<I: Index>(
        &self,
        render_pass: &mut wgpu::RenderPass,
        index_buffer: &IndexBuffer<I>,
    ) {
        render_pass.set_vertex_buffer(0, self.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), I::FORMAT);
        render_pass.draw_indexed(0..index_buffer.len(), 0, 0..1);
    }

    pub fn draw_instanced<E>(
        &self,
        render_pass: &mut wgpu::RenderPass,
        instance_buffer: &InstanceBuffer<E>,
    ) {
        render_pass.set_vertex_buffer(0, self.slice(..));
        render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
        render_pass.draw(0..self.len(), 0..instance_buffer.len());
    }
}

impl<V: Pod> VertexBuffer<V> {
    pub fn new(renderer: &Renderer, state: MemoryState<[V], usize>) -> Self {
        Self(Buffer::<[_]>::new(
            renderer,
            state.data(),
            state.usage(wgpu::BufferUsages::VERTEX),
        ))
    }

    pub fn new_non_empty(renderer: &Renderer, state: MemoryState<[V], usize>) -> Option<Self> {
        (!state.is_empty()).then(|| Self::new(renderer, state))
    }
}

impl<V> Deref for VertexBuffer<V> {
    type Target = Buffer<[V]>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct IndexBuffer<I>(Buffer<[I]>);

impl<I: Pod> IndexBuffer<I> {
    pub fn new(renderer: &Renderer, state: MemoryState<[I], usize>) -> Self {
        Self(Buffer::<[_]>::new(
            renderer,
            state.data(),
            state.usage(wgpu::BufferUsages::INDEX),
        ))
    }
}

impl<I> Deref for IndexBuffer<I> {
    type Target = Buffer<[I]>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct InstanceBuffer<E>(Buffer<[E]>);

impl<E: Pod> InstanceBuffer<E> {
    pub fn new(renderer: &Renderer, state: MemoryState<[E], usize>) -> Self {
        Self(Buffer::<[_]>::new(
            renderer,
            state.data(),
            state.usage(wgpu::BufferUsages::VERTEX),
        ))
    }
}

impl<E> Deref for InstanceBuffer<E> {
    type Target = Buffer<[E]>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct UniformBuffer<T>(Buffer<T>);

impl<T: Pod> UniformBuffer<T> {
    pub fn new(renderer: &Renderer, state: MemoryState<T>) -> Self {
        Self(Buffer::<T>::new(
            renderer,
            state.value(),
            state.usage(wgpu::BufferUsages::UNIFORM),
        ))
    }
}

impl<T> Deref for UniformBuffer<T> {
    type Target = Buffer<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct Buffer<T: ?Sized> {
    buffer: wgpu::Buffer,
    phantom: PhantomData<T>,
}

impl<T: Pod> Buffer<T> {
    pub fn new(renderer: &Renderer, value: Option<&T>, usage: wgpu::BufferUsages) -> Self {
        Self {
            buffer: Buffer::<[_]>::new(renderer, value.map(slice::from_ref).ok_or(1), usage).buffer,
            phantom: PhantomData,
        }
    }
}

impl<T: Pod> Buffer<[T]> {
    pub fn new(
        Renderer { device, .. }: &Renderer,
        data: Result<&[T], usize>,
        usage: wgpu::BufferUsages,
    ) -> Self {
        Self {
            buffer: match data {
                Ok(data) => device.create_buffer_init(&BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(data),
                    usage,
                }),
                Err(len) => device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: (len * size_of::<T>()) as u64,
                    usage,
                    mapped_at_creation: false,
                }),
            },
            phantom: PhantomData,
        }
    }
}

impl<T> Buffer<[T]> {
    pub fn len(&self) -> u32 {
        (self.buffer.size() / size_of::<T>() as u64) as u32
    }
}

impl<T: Pod> Buffer<T> {
    pub fn set(&self, Renderer { queue, .. }: &Renderer, value: &T) {
        queue.write_buffer(
            &self.buffer,
            0,
            bytemuck::cast_slice(slice::from_ref(value)),
        );
    }
}

impl<T: Pod> Buffer<[T]> {
    pub fn write(&self, Renderer { queue, .. }: &Renderer, data: &[T]) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(data));
    }
}

impl<T: ?Sized> Deref for Buffer<T> {
    type Target = wgpu::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

pub enum MemoryState<'a, T: ?Sized, U = ()> {
    Immutable(&'a T),
    Uninit(U),
}

impl<T: ?Sized, U> MemoryState<'_, T, U> {
    fn usage(&self, usage: wgpu::BufferUsages) -> wgpu::BufferUsages {
        if matches!(self, Self::Uninit(_)) {
            usage | wgpu::BufferUsages::COPY_DST
        } else {
            usage
        }
    }
}

impl<'a, T> MemoryState<'a, T, ()> {
    pub const UNINIT: Self = Self::Uninit(());

    fn value(self) -> Option<&'a T> {
        if let Self::Immutable(value) = self {
            Some(value)
        } else {
            None
        }
    }
}

impl<'a, T> MemoryState<'a, [T], usize> {
    fn data(self) -> Result<&'a [T], usize> {
        match self {
            Self::Immutable(data) => Ok(data),
            Self::Uninit(len) => Err(len),
        }
    }

    fn is_empty(self) -> bool {
        match self {
            Self::Immutable(data) => data.is_empty(),
            Self::Uninit(len) => len == 0,
        }
    }
}

impl<T: ?Sized, U: Clone> Clone for MemoryState<'_, T, U> {
    fn clone(&self) -> Self {
        match self {
            Self::Immutable(data) => Self::Immutable(data),
            Self::Uninit(fallback) => Self::Uninit(fallback.clone()),
        }
    }
}

impl<T: ?Sized, U: Copy> Copy for MemoryState<'_, T, U> {}

pub trait Vertex: Pod {
    const ATTRIBS: &'static [wgpu::VertexAttribute];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
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

pub trait Instance: Pod {
    const ATTRIBS: &'static [wgpu::VertexAttribute];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: Self::ATTRIBS,
        }
    }
}
