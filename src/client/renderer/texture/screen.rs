use crate::client::{
    event_loop::{Event, EventHandler},
    renderer::Renderer,
};
use std::{
    array,
    ops::{Deref, DerefMut},
};

pub struct ScreenTexture(ScreenTextureArray<1>);

impl ScreenTexture {
    pub fn new(renderer: &Renderer, format: wgpu::TextureFormat) -> Self {
        Self(ScreenTextureArray::new(renderer, format))
    }

    pub fn view(&self) -> &wgpu::TextureView {
        self.0.view(0)
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.0.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.0.bind_group(0)
    }
}

impl EventHandler for ScreenTexture {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.0.handle(event, renderer);
    }
}

pub struct ScreenTextureArray<const N: usize> {
    views: [wgpu::TextureView; N],
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_groups: [wgpu::BindGroup; N],
    format: wgpu::TextureFormat,
}

impl<const N: usize> ScreenTextureArray<N> {
    pub fn new(renderer @ Renderer { device, .. }: &Renderer, format: wgpu::TextureFormat) -> Self {
        let views = Self::create_views(renderer, format);
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
        let bind_groups = Self::create_bind_groups(renderer, &views, &sampler, &bind_group_layout);
        Self {
            views,
            sampler,
            bind_group_layout,
            bind_groups,
            format,
        }
    }

    pub fn view(&self, index: usize) -> &wgpu::TextureView {
        &self.views[index]
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self, index: usize) -> &wgpu::BindGroup {
        &self.bind_groups[index]
    }

    fn create_views(
        Renderer { device, config, .. }: &Renderer,
        format: wgpu::TextureFormat,
    ) -> [wgpu::TextureView; N] {
        array::from_fn(|_| {
            device
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
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        })
    }

    fn create_bind_groups(
        Renderer { device, .. }: &Renderer,
        views: &[wgpu::TextureView; N],
        sampler: &wgpu::Sampler,
        layout: &wgpu::BindGroupLayout,
    ) -> [wgpu::BindGroup; N] {
        array::from_fn(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&views[i]),
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

impl ScreenTextureArray<2> {
    pub fn swap(&mut self) {
        self.views.swap(0, 1);
        self.bind_groups.swap(0, 1);
    }
}

impl<const N: usize> EventHandler for ScreenTextureArray<N> {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, _: &Event, renderer: Self::Context<'_>) {
        if renderer.is_resized {
            self.views = Self::create_views(renderer, self.format);
            self.bind_groups = Self::create_bind_groups(
                renderer,
                &self.views,
                &self.sampler,
                &self.bind_group_layout,
            );
        }
    }
}

pub struct DepthBuffer(ScreenTexture);

impl DepthBuffer {
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(renderer: &Renderer) -> Self {
        Self(ScreenTexture::new(renderer, Self::FORMAT))
    }
}

impl Deref for DepthBuffer {
    type Target = ScreenTexture;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for DepthBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
