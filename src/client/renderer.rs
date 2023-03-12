use super::{
    event_loop::{Event, EventHandler},
    window::Window,
};
use bytemuck::Pod;
use image::RgbaImage;
use nalgebra::Point3;
use std::{
    array,
    marker::PhantomData,
    mem,
    num::{NonZeroU32, NonZeroU8},
    slice,
};
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
        let PhysicalSize { width, height } = window.as_ref().inner_size();
        let instance = wgpu::Instance::default();
        let surface = unsafe {
            instance
                .create_surface(window.as_ref())
                .expect("surface should be creatable")
        };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("adapter should be available");
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::PUSH_CONSTANTS
                        | wgpu::Features::TEXTURE_BINDING_ARRAY
                        | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
                    limits: wgpu::Limits {
                        max_push_constant_size: 128,
                        ..Default::default()
                    },
                },
                None,
            )
            .await
            .expect("device should be available");
        let config = wgpu::SurfaceConfiguration {
            present_mode: wgpu::PresentMode::Fifo,
            ..surface
                .get_default_config(&adapter, width, height)
                .expect("surface should be supported by adapter")
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
        self.surface.configure(&self.device, &self.config);
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

pub struct Mesh<V> {
    vertex_buffer: wgpu::Buffer,
    phantom: PhantomData<V>,
}

impl<V: Vertex> Mesh<V> {
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
        render_pass.set_index_buffer(self.index_buffer.slice(..), I::FORMAT);
        render_pass.draw_indexed(0..self.len(), 0, 0..1);
    }

    fn len(&self) -> u32 {
        (self.index_buffer.size() / mem::size_of::<I>() as u64) as u32
    }
}

pub struct Uniform<T> {
    buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    phantom: PhantomData<T>,
}

impl<T: Pod> Uniform<T> {
    pub fn new(
        renderer @ Renderer { device, .. }: &Renderer,
        visibility: wgpu::ShaderStages,
    ) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: mem::size_of::<T>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = Self::create_bind_group_layout(renderer, visibility);
        let bind_group = Self::create_bind_group(renderer, &buffer, &bind_group_layout);
        Self {
            buffer,
            bind_group_layout,
            bind_group,
            phantom: PhantomData,
        }
    }

    pub fn with_constant_data(
        renderer @ Renderer { device, .. }: &Renderer,
        data: &T,
        visibility: wgpu::ShaderStages,
    ) -> Self {
        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(slice::from_ref(data)),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group_layout = Self::create_bind_group_layout(renderer, visibility);
        let bind_group = Self::create_bind_group(renderer, &buffer, &bind_group_layout);
        Self {
            buffer,
            bind_group_layout,
            bind_group,
            phantom: PhantomData,
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn write(&self, Renderer { queue, .. }: &Renderer, data: &T) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(slice::from_ref(data)));
    }

    fn create_bind_group_layout(
        Renderer { device, .. }: &Renderer,
        visibility: wgpu::ShaderStages,
    ) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }

    fn create_bind_group(
        Renderer { device, .. }: &Renderer,
        buffer: &wgpu::Buffer,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    }
}

pub struct ImageTexture {
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl ImageTexture {
    pub fn new(
        renderer @ Renderer { device, .. }: &Renderer,
        bytes: &[u8],
        is_srgb: bool,
        is_pixelated: bool,
        mipmap_levels: u32,
    ) -> Self {
        let view = Self::create_view(renderer, bytes, is_srgb, mipmap_levels);
        let sampler = Self::create_sampler(renderer, is_pixelated, mipmap_levels);
        let bind_group_layout = Self::create_bind_group_layout(renderer, is_pixelated, None);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Self {
            bind_group_layout,
            bind_group,
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    fn create_view(
        renderer @ Renderer {
            device,
            queue,
            config,
            ..
        }: &Renderer,
        bytes: &[u8],
        is_srgb: bool,
        mipmap_levels: u32,
    ) -> wgpu::TextureView {
        let image = Self::load_rgba(bytes);
        let (width, height) = image.dimensions();
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: mipmap_levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: if is_srgb && config.format.describe().srgb {
                wgpu::TextureFormat::Rgba8UnormSrgb
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            },
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &image,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: NonZeroU32::new(4 * width),
                rows_per_image: NonZeroU32::new(height),
            },
            size,
        );

        if mipmap_levels > 1 {
            Self::generate_mipmaps(renderer, &texture, mipmap_levels);
        }

        texture.create_view(&Default::default())
    }

    fn create_sampler(
        Renderer { device, .. }: &Renderer,
        is_pixelated: bool,
        mipmap_levels: u32,
    ) -> wgpu::Sampler {
        device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: if is_pixelated {
                wgpu::FilterMode::Nearest
            } else {
                wgpu::FilterMode::Linear
            },
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Linear,
            anisotropy_clamp: if mipmap_levels > 1 {
                NonZeroU8::new(16)
            } else {
                None
            },
            ..Default::default()
        })
    }

    fn create_bind_group_layout(
        Renderer { device, .. }: &Renderer,
        is_pixelated: bool,
        count: Option<NonZeroU32>,
    ) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float {
                            filterable: !is_pixelated,
                        },
                    },
                    count,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(if is_pixelated {
                        wgpu::SamplerBindingType::NonFiltering
                    } else {
                        wgpu::SamplerBindingType::Filtering
                    }),
                    count: None,
                },
            ],
        })
    }

    fn generate_mipmaps(
        renderer @ Renderer { device, queue, .. }: &Renderer,
        texture: &wgpu::Texture,
        levels: u32,
    ) {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let blit = Blit::new(renderer, &bind_group_layout, Some(texture.format()));
        let mut encoder = device.create_command_encoder(&Default::default());

        (0..levels)
            .map(|level| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    base_mip_level: level,
                    mip_level_count: NonZeroU32::new(1),
                    ..Default::default()
                })
            })
            .collect::<Vec<_>>()
            .windows(2)
            .for_each(|views| {
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&views[0]),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                });
                blit.draw(
                    &mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &views[1],
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: None,
                    }),
                    &bind_group,
                );
            });

        queue.submit([encoder.finish()]);
    }

    fn load_rgba(bytes: &[u8]) -> RgbaImage {
        image::load_from_memory(bytes)
            .expect("bytes should be a valid image")
            .to_rgba8()
    }
}

pub struct ImageTextureArray {
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl ImageTextureArray {
    pub fn new(
        renderer @ Renderer { device, .. }: &Renderer,
        data: impl IntoIterator<Item = impl AsRef<[u8]>>,
        is_srgb: bool,
        is_pixelated: bool,
        mipmap_levels: u32,
    ) -> Self {
        let views = Self::create_views(renderer, data, is_srgb, mipmap_levels);
        let sampler = ImageTexture::create_sampler(renderer, is_pixelated, mipmap_levels);
        let bind_group_layout = ImageTexture::create_bind_group_layout(
            renderer,
            is_pixelated,
            NonZeroU32::new(views.len() as u32),
        );
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureViewArray(
                        &views.iter().collect::<Vec<_>>(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Self {
            bind_group_layout,
            bind_group,
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    fn create_views(
        renderer: &Renderer,
        data: impl IntoIterator<Item = impl AsRef<[u8]>>,
        is_srgb: bool,
        mipmap_levels: u32,
    ) -> Vec<wgpu::TextureView> {
        data.into_iter()
            .map(|bytes| {
                ImageTexture::create_view(renderer, bytes.as_ref(), is_srgb, mipmap_levels)
            })
            .collect::<Vec<_>>()
    }
}

pub struct ScreenTexture {
    view: wgpu::TextureView,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
    is_resized: bool,
}

impl ScreenTexture {
    pub fn new(
        Renderer { device, config, .. }: &Renderer,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> Self {
        Self {
            view: device
                .create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width: config.width,
                        height: config.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default()),
            format,
            usage,
            is_resized: false,
        }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

impl EventHandler for ScreenTexture {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
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
                self.is_resized = true;
            }
            Event::RedrawRequested(_) if self.is_resized => {
                *self = Self::new(renderer, self.format, self.usage);
            }
            _ => {}
        }
    }
}

pub struct DepthBuffer {
    texture: ScreenTexture,
}

impl DepthBuffer {
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(renderer: &Renderer) -> Self {
        Self {
            texture: ScreenTexture::new(
                renderer,
                Self::FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            ),
        }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        self.texture.view()
    }
}

impl EventHandler for DepthBuffer {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.texture.handle(event, renderer);
    }
}

pub struct InputOutputTextureArray<const N: usize> {
    textures: [ScreenTexture; N],
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_groups: [wgpu::BindGroup; N],
    is_resized: bool,
}

impl<const N: usize> InputOutputTextureArray<N> {
    #[rustfmt::skip]
    pub fn new(renderer @ Renderer { device, config, .. }: &Renderer) -> Self {
        let textures = array::from_fn(|_| {
            ScreenTexture::new(
                renderer,
                config.format,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            )
        });
        let sampler = device.create_sampler(&Default::default());
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });
        let bind_groups = Self::create_bind_groups(renderer, &textures, &sampler, &bind_group_layout);
        Self {
            textures,
            sampler,
            bind_group_layout,
            bind_groups,
            is_resized: false,
        }
    }

    pub fn view(&self, index: usize) -> &wgpu::TextureView {
        self.textures[index].view()
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self, index: usize) -> &wgpu::BindGroup {
        &self.bind_groups[index]
    }

    fn create_bind_groups(
        Renderer { device, .. }: &Renderer,
        textures: &[ScreenTexture; N],
        sampler: &wgpu::Sampler,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> [wgpu::BindGroup; N] {
        array::from_fn(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(textures[i].view()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            })
        })
    }
}

impl<const N: usize> EventHandler for InputOutputTextureArray<N> {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        for texture in &mut self.textures {
            texture.handle(event, renderer);
        }

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
                self.is_resized = true;
            }
            Event::RedrawRequested(_) if self.is_resized => {
                self.bind_groups = Self::create_bind_groups(
                    renderer,
                    &self.textures,
                    &self.sampler,
                    &self.bind_group_layout,
                );
            }
            Event::RedrawEventsCleared => {
                self.is_resized = false;
            }
            _ => {}
        }
    }
}

pub struct PostProcessor {
    textures: InputOutputTextureArray<2>,
    blit: Blit,
    index: bool,
}

impl PostProcessor {
    pub fn new(renderer: &Renderer) -> Self {
        let textures = InputOutputTextureArray::new(renderer);
        let blit = Blit::new(renderer, textures.bind_group_layout(), None);
        Self {
            textures,
            blit,
            index: false,
        }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        self.main_view()
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.textures.bind_group_layout()
    }

    pub fn apply<E: Effect>(&mut self, encoder: &mut wgpu::CommandEncoder, effect: &E) {
        effect.draw(
            &mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.secondary_view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            }),
            self.main_bind_group(),
        );
        self.swap();
    }

    pub fn draw(&self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        self.blit.draw(
            &mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            }),
            self.main_bind_group(),
        );
    }

    fn swap(&mut self) {
        self.index = !self.index;
    }

    fn main_view(&self) -> &wgpu::TextureView {
        self.textures.view(self.main_index())
    }

    fn secondary_view(&self) -> &wgpu::TextureView {
        self.textures.view(self.secondary_index())
    }

    fn main_bind_group(&self) -> &wgpu::BindGroup {
        self.textures.bind_group(self.main_index())
    }

    fn main_index(&self) -> usize {
        self.index.into()
    }

    fn secondary_index(&self) -> usize {
        (!self.index).into()
    }
}

impl EventHandler for PostProcessor {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.textures.handle(event, renderer);
    }
}

pub struct Program {
    render_pipeline: wgpu::RenderPipeline,
}

impl Program {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        Renderer { device, config, .. }: &Renderer,
        desc: wgpu::ShaderModuleDescriptor,
        buffers: &[wgpu::VertexBufferLayout],
        bind_group_layouts: &[&wgpu::BindGroupLayout],
        push_constant_ranges: &[wgpu::PushConstantRange],
        format: Option<wgpu::TextureFormat>,
        blend: Option<wgpu::BlendState>,
        cull_mode: Option<wgpu::Face>,
        depth_stencil: Option<wgpu::DepthStencilState>,
    ) -> Self {
        let shader = device.create_shader_module(desc);
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts,
                push_constant_ranges,
            });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: format.unwrap_or(config.format),
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode,
                ..Default::default()
            },
            depth_stencil,
            multisample: Default::default(),
            multiview: None,
        });
        Self { render_pipeline }
    }

    pub fn bind<'a, I>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, bind_groups: I)
    where
        I: IntoIterator<Item = &'a wgpu::BindGroup>,
    {
        render_pass.set_pipeline(&self.render_pipeline);
        for (bind_group, i) in bind_groups.into_iter().zip(0..) {
            render_pass.set_bind_group(i, bind_group, &[]);
        }
    }
}

pub struct Blit(Program);

impl Blit {
    pub fn new(
        renderer: &Renderer,
        input_bind_group_layout: &wgpu::BindGroupLayout,
        format: Option<wgpu::TextureFormat>,
    ) -> Self {
        Self(Program::new(
            renderer,
            wgpu::include_wgsl!("../../assets/shaders/blit.wgsl"),
            &[],
            &[input_bind_group_layout],
            &[],
            format,
            None,
            None,
            None,
        ))
    }
}

impl Effect for Blit {
    fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        input_bind_group: &'a wgpu::BindGroup,
    ) {
        self.0.bind(render_pass, [input_bind_group]);
        render_pass.draw(0..3, 0..1);
    }
}

pub trait Vertex: Pod {
    const ATTRIBS: &'static [wgpu::VertexAttribute];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: Self::ATTRIBS,
        }
    }
}

impl Vertex for Point3<f32> {
    const ATTRIBS: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![0 => Float32x3];
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

pub trait Effect {
    fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        input_bind_group: &'a wgpu::BindGroup,
    );
}
