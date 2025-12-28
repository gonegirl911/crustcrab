use crate::client::renderer::{
    Renderer,
    effect::{Blit, Effect},
};
use bon::bon;
use image::RgbaImage;
use std::num::NonZero;

pub struct ImageTexture {
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

#[bon]
impl ImageTexture {
    #[builder]
    pub fn new<R: AsRgbaImage>(
        renderer @ Renderer { device, .. }: &Renderer,
        image: R,
        #[builder(default = 1)] mip_level_count: u32,
        is_srgb: bool,
        #[builder(default)] address_mode: wgpu::AddressMode,
    ) -> Self {
        let view = Self::create_view(renderer, image, mip_level_count, is_srgb);
        let sampler = Self::create_sampler(renderer, address_mode, mip_level_count);
        let bind_group_layout = ImageTextureArray::create_bind_group_layout(renderer, &[]);
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

    fn create_view<R: AsRgbaImage>(
        renderer @ Renderer {
            device,
            queue,
            config,
            ..
        }: &Renderer,
        image: R,
        mip_level_count: u32,
        is_srgb: bool,
    ) -> wgpu::TextureView {
        let image = image.as_rgba_image();
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
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            image,
            wgpu::TexelCopyBufferLayout {
                bytes_per_row: Some(4 * width),
                ..Default::default()
            },
            size,
        );

        if mip_level_count > 1 {
            Self::generate_mip_levels(renderer, &texture, mip_level_count);
        }

        texture.create_view(&Default::default())
    }

    fn create_sampler(
        Renderer { device, .. }: &Renderer,
        address_mode: wgpu::AddressMode,
        mip_level_count: u32,
    ) -> wgpu::Sampler {
        device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            mipmap_filter: if mip_level_count > 1 {
                wgpu::MipmapFilterMode::Linear
            } else {
                wgpu::MipmapFilterMode::Nearest
            },
            ..Default::default()
        })
    }

    fn generate_mip_levels(
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
        let mut views = (0..count).map(|level| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: level,
                mip_level_count: Some(1),
                ..Default::default()
            })
        });
        let mut src = views.next().unwrap_or_else(|| unreachable!());

        for dst in views {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&src),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });
            blit.draw(
                &mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &dst,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(Default::default()),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                }),
                &bind_group,
            );
            src = dst;
        }

        queue.submit([encoder.finish()]);
    }
}

pub struct ImageTextureArray {
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

#[bon]
impl ImageTextureArray {
    #[builder]
    pub fn new<R: IntoIterator<Item: AsRgbaImage>>(
        renderer @ Renderer { device, .. }: &Renderer,
        images: R,
        #[builder(default = 1)] mip_level_count: u32,
        is_srgb: bool,
        #[builder(default)] address_mode: wgpu::AddressMode,
    ) -> Self {
        let views = Self::create_views(renderer, images, mip_level_count, is_srgb);
        let sampler = ImageTexture::create_sampler(renderer, address_mode, mip_level_count);
        let bind_group_layout = Self::create_bind_group_layout(renderer, &views);
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

    fn create_views<R: IntoIterator<Item: AsRgbaImage>>(
        renderer: &Renderer,
        images: R,
        mip_level_count: u32,
        is_srgb: bool,
    ) -> Vec<wgpu::TextureView> {
        images
            .into_iter()
            .map(|image| ImageTexture::create_view(renderer, image, mip_level_count, is_srgb))
            .collect()
    }

    fn create_bind_group_layout(
        Renderer { device, .. }: &Renderer,
        views: &[wgpu::TextureView],
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
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: NonZero::new(views.len() as u32),
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }
}

pub trait AsRgbaImage {
    fn as_rgba_image(&self) -> &RgbaImage;
}

impl AsRgbaImage for RgbaImage {
    fn as_rgba_image(&self) -> &RgbaImage {
        self
    }
}

impl AsRgbaImage for &RgbaImage {
    fn as_rgba_image(&self) -> &RgbaImage {
        self
    }
}
