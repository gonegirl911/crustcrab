use crate::client::{
    event_loop::{Event, EventHandler},
    renderer::Renderer,
};
use std::array;
use winit::{dpi::PhysicalSize, event::WindowEvent};

struct ScreenTexture {
    view: wgpu::TextureView,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
    is_resized: bool,
}

impl ScreenTexture {
    fn new(
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

    fn view(&self) -> &wgpu::TextureView {
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
            Event::RedrawRequested(_) => {
                if self.is_resized {
                    *self = Self::new(renderer, self.format, self.usage);
                }
            }
            _ => {}
        }
    }
}

struct InputOutputTexture(InputOutputTextureArray<1>);

impl InputOutputTexture {
    fn new(renderer: &Renderer, format: wgpu::TextureFormat) -> Self {
        Self(InputOutputTextureArray::new(renderer, format))
    }

    fn view(&self) -> &wgpu::TextureView {
        self.0.view(0)
    }

    fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.0.bind_group_layout()
    }

    fn bind_group(&self) -> &wgpu::BindGroup {
        self.0.bind_group(0)
    }
}

impl EventHandler for InputOutputTexture {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.0.handle(event, renderer);
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
    pub fn new(renderer @ Renderer { device, .. }: &Renderer, format: wgpu::TextureFormat) -> Self {
        let textures = array::from_fn(|_| {
            ScreenTexture::new(
                renderer,
                format,
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
        let bind_groups =
            Self::create_bind_groups(renderer, &textures, &sampler, &bind_group_layout);
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

impl InputOutputTextureArray<2> {
    pub fn swap(&mut self) {
        self.textures.swap(0, 1);
        self.bind_groups.swap(0, 1);
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

pub struct DepthBuffer(InputOutputTexture);

impl DepthBuffer {
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(renderer: &Renderer) -> Self {
        Self(InputOutputTexture::new(renderer, Self::FORMAT))
    }

    pub fn view(&self) -> &wgpu::TextureView {
        self.0.view()
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.0.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.0.bind_group()
    }
}

impl EventHandler for DepthBuffer {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.0.handle(event, renderer);
    }
}
