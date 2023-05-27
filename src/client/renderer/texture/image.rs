use crate::client::renderer::{
    effect::{Blit, Effect},
    Renderer,
};
use image::{io::Reader as ImageReader, RgbaImage};
use std::{num::NonZeroU32, path::Path};

pub struct ImageTexture {
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl ImageTexture {
    pub fn new<P: AsRef<Path>>(
        renderer @ Renderer { device, .. }: &Renderer,
        path: P,
        is_srgb: bool,
        is_pixelated: bool,
        mip_level_count: u32,
    ) -> Self {
        let view = Self::create_view(renderer, path, is_srgb, mip_level_count);
        let sampler = Self::create_sampler(renderer, is_pixelated, mip_level_count);
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

    fn create_view<P: AsRef<Path>>(
        renderer @ Renderer {
            device,
            queue,
            config,
            ..
        }: &Renderer,
        path: P,
        is_srgb: bool,
        mip_level_count: u32,
    ) -> wgpu::TextureView {
        let image = Self::load_rgba(path);
        let (width, height) = image.dimensions();
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: if is_srgb && config.format.is_srgb() {
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
                bytes_per_row: Some(width * 4),
                ..Default::default()
            },
            size,
        );

        if mip_level_count > 1 {
            Self::gen_mip_levels(renderer, &texture, mip_level_count);
        }

        texture.create_view(&Default::default())
    }

    fn create_sampler(
        Renderer { device, .. }: &Renderer,
        is_pixelated: bool,
        mip_level_count: u32,
    ) -> wgpu::Sampler {
        device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: if is_pixelated {
                wgpu::FilterMode::Nearest
            } else {
                wgpu::FilterMode::Linear
            },
            mipmap_filter: if mip_level_count > 1 {
                wgpu::FilterMode::Linear
            } else {
                wgpu::FilterMode::Nearest
            },
            anisotropy_clamp: 1,
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

    fn gen_mip_levels(
        renderer @ Renderer { device, queue, .. }: &Renderer,
        texture: &wgpu::Texture,
        count: u32,
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
        let blit = Blit::new(renderer, &bind_group_layout, texture.format());
        let mut encoder = device.create_command_encoder(&Default::default());

        (0..count)
            .map(|level| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    base_mip_level: level,
                    mip_level_count: Some(1),
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

    fn load_rgba<P: AsRef<Path>>(path: P) -> RgbaImage {
        ImageReader::open(path)
            .expect("file should exist")
            .decode()
            .expect("format should be valid")
            .into_rgba8()
    }
}

pub struct ImageTextureArray {
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl ImageTextureArray {
    pub fn new(
        renderer @ Renderer { device, .. }: &Renderer,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
        is_srgb: bool,
        is_pixelated: bool,
        mip_level_count: u32,
    ) -> Self {
        let views = Self::create_views(renderer, paths, is_srgb, mip_level_count);
        let sampler = ImageTexture::create_sampler(renderer, is_pixelated, mip_level_count);
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
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
        is_srgb: bool,
        mip_level_count: u32,
    ) -> Vec<wgpu::TextureView> {
        paths
            .into_iter()
            .map(|path| ImageTexture::create_view(renderer, path, is_srgb, mip_level_count))
            .collect::<Vec<_>>()
    }
}
