use super::{
    event_loop::{Event, EventHandler},
    window::Window,
};
use bytemuck::Pod;
use std::{marker::PhantomData, mem, slice};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use winit::{dpi::PhysicalSize, event::WindowEvent};

pub struct Renderer {
    pub surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    is_resized: bool,
}

impl Renderer {
    pub async fn new(window: &Window) -> Self {
        let size = window.as_ref().inner_size();
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window.as_ref()) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("wgpu adapter should be available");
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::PUSH_CONSTANTS,
                    limits: wgpu::Limits {
                        max_push_constant_size: 128,
                        ..Default::default()
                    },
                },
                None,
            )
            .await
            .expect("wgpu device should be available");
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_supported_formats(&adapter)[0],
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
        };
        Self {
            surface,
            device,
            queue,
            config,
            is_resized: true,
        }
    }

    pub fn recreate_surface(&self) {
        self.surface.configure(&self.device, &self.config)
    }
}

impl EventHandler for Renderer {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, _: Self::Context<'_>) {
        match event {
            Event::WindowEvent {
                event:
                    WindowEvent::Resized(PhysicalSize { width, height })
                    | WindowEvent::ScaleFactorChanged {
                        new_inner_size: PhysicalSize { width, height },
                        ..
                    },
                ..
            } if *width != 0 && *height != 0 => {
                self.config.width = *width;
                self.config.height = *height;
                self.is_resized = true;
            }
            Event::RedrawRequested(_) if self.is_resized => {
                self.recreate_surface();
            }
            Event::RedrawEventsCleared => {
                self.is_resized = false;
            }
            _ => {}
        }
    }
}

pub trait Viewable {
    fn view(&self) -> &wgpu::TextureView;
}

pub trait Bindable {
    fn bind_group_layout(&self) -> &wgpu::BindGroupLayout;
    fn bind_group(&self) -> &wgpu::BindGroup;
}

pub trait Vertex: Pod {
    const ATTRIBS: &'static [wgpu::VertexAttribute];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3],
        }
    }
}

pub trait Index: Pod {
    fn format() -> wgpu::IndexFormat;
}

impl Index for u16 {
    fn format() -> wgpu::IndexFormat {
        wgpu::IndexFormat::Uint16
    }
}

impl Index for u32 {
    fn format() -> wgpu::IndexFormat {
        wgpu::IndexFormat::Uint32
    }
}

pub struct Uniform<T> {
    buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    phantom: PhantomData<T>,
}

impl<T: Pod> Uniform<T> {
    pub fn new(Renderer { device, .. }: &Renderer) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: mem::size_of::<T>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        Self {
            buffer,
            bind_group_layout,
            bind_group,
            phantom: PhantomData,
        }
    }

    pub fn update(&self, Renderer { queue, .. }: &Renderer, data: &T) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(slice::from_ref(data)))
    }
}

impl<T> Bindable for Uniform<T> {
    fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}

pub struct Mesh<V> {
    vertex_buffer: wgpu::Buffer,
    phantom: PhantomData<V>,
}

impl<V: Pod> Mesh<V> {
    pub fn new(Renderer { device, .. }: &Renderer, vertices: &[V]) -> Self {
        Self {
            vertex_buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            phantom: PhantomData,
        }
    }

    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..self.len(), 0..1);
    }

    fn len(&self) -> u32 {
        (self.vertex_buffer.size() / mem::size_of::<V>() as u64) as u32
    }
}

pub struct IndexedMesh<V, I> {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    phantom: PhantomData<(V, I)>,
}

impl<V: Vertex, I: Index> IndexedMesh<V, I> {
    pub fn new(Renderer { device, .. }: &Renderer, vertices: &[V], indices: &[I]) -> Self {
        Self {
            vertex_buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            index_buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            phantom: PhantomData,
        }
    }

    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), I::format());
        render_pass.draw_indexed(0..self.len(), 0, 0..1);
    }

    fn len(&self) -> u32 {
        (self.index_buffer.size() / mem::size_of::<I>() as u64) as u32
    }
}
