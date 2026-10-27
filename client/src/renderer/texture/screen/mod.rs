pub(crate) mod depth;

use crate::{
    event::{self, Event},
    renderer::{Renderer, Surface},
};
use std::array;

pub(crate) struct ScreenTexture(ScreenTextureArray<1>);

impl ScreenTexture {
    pub(crate) fn new(renderer: &Renderer, surface: &Surface, format: wgpu::TextureFormat) -> Self {
        Self(ScreenTextureArray::new(renderer, surface, format))
    }

    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.0.views[0]
    }

    pub(crate) fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.0.bind_group_layout
    }

    pub(crate) fn bind_group(&self) -> &wgpu::BindGroup {
        &self.0.bind_groups[0]
    }
}

impl event::Handler for ScreenTexture {
    type Context<'a> = (&'a Renderer, &'a Surface);

    fn handle(&mut self, event: &Event, (renderer, surface): Self::Context<'_>) {
        self.0.handle(event, (renderer, surface));
    }
}

pub(in crate::renderer) struct ScreenTextureArray<const N: usize> {
    pub(in crate::renderer) views: [wgpu::TextureView; N],
    sampler: wgpu::Sampler,
    pub(in crate::renderer) mut(self) bind_group_layout: wgpu::BindGroupLayout,
    pub(in crate::renderer) bind_groups: [wgpu::BindGroup; N],
}

impl<const N: usize> ScreenTextureArray<N> {
    pub(in crate::renderer) fn new(
        renderer @ Renderer { device, .. }: &Renderer,
        surface: &Surface,
        format: wgpu::TextureFormat,
    ) -> Self {
        const { assert!(N != 0) };

        let textures = array::from_fn(|_| Self::create_texture(renderer, surface, format));
        let views = textures.each_ref().map(Self::create_view);
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
        let bind_groups = views
            .each_ref()
            .map(|view| Self::create_bind_group(renderer, view, &sampler, &bind_group_layout));
        Self {
            views,
            sampler,
            bind_group_layout,
            bind_groups,
        }
    }

    pub(in crate::renderer) fn swap(&mut self, a: usize, b: usize) {
        self.views.swap(a, b);
        self.bind_groups.swap(a, b);
    }

    fn format(&self) -> wgpu::TextureFormat {
        self.views[0].texture().format()
    }

    fn create_texture(
        Renderer { device, .. }: &Renderer,
        surface: &Surface,
        format: wgpu::TextureFormat,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: surface.config.width,
                height: surface.config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    fn create_view(texture: &wgpu::Texture) -> wgpu::TextureView {
        texture.create_view(&Default::default())
    }

    fn create_bind_group(
        Renderer { device, .. }: &Renderer,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}

impl<const N: usize> event::Handler for ScreenTextureArray<N> {
    type Context<'a> = (&'a Renderer, &'a Surface);

    fn handle(&mut self, _: &Event, (renderer, surface): Self::Context<'_>) {
        if surface.is_resized() {
            let format = self.format();
            let textures = array::from_fn(|_| Self::create_texture(renderer, surface, format));
            self.views = textures.each_ref().map(Self::create_view);
            self.bind_groups = self.views.each_ref().map(|view| {
                Self::create_bind_group(renderer, view, &self.sampler, &self.bind_group_layout)
            });
        }
    }
}
