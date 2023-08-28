use super::Renderer;
use bytemuck::Pod;
use std::{marker::PhantomData, mem, ops::Deref, slice};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

pub struct Buffer<T: ?Sized> {
    buffer: wgpu::Buffer,
    phantom: PhantomData<T>,
}

impl<T: Pod> Buffer<T> {
    pub fn new(
        Renderer { device, .. }: &Renderer,
        value: Option<&T>,
        usage: wgpu::BufferUsages,
    ) -> Self {
        Self {
            buffer: if let Some(value) = value {
                device.create_buffer_init(&BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(slice::from_ref(value)),
                    usage,
                })
            } else {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: mem::size_of::<T>() as u64,
                    usage,
                    mapped_at_creation: false,
                })
            },
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
                    size: (len * mem::size_of::<T>()) as u64,
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
        (self.buffer.size() / mem::size_of::<T>() as u64) as u32
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
