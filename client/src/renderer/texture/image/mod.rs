mod mip;

use crate::renderer::Renderer;
use bon::bon;
use image::RgbaImage;
use std::{num::NonZero, path::Path};

pub(crate) struct ImageTexture {
    pub(crate) mut(self) bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) mut(self) bind_group: wgpu::BindGroup,
}

#[bon]
impl ImageTexture {
    #[builder]
    pub(crate) fn new(
        renderer @ Renderer { device, .. }: &Renderer,
        image: &RgbaImage,
        #[builder(default = 1)] mip_level_count: u32,
        is_srgb: bool,
        #[builder(default)] address_mode: wgpu::AddressMode,
    ) -> Self {
        let texture = Self::create_texture(renderer, image, mip_level_count, is_srgb);
        ImageTextureArray::generate_mips(renderer, [&texture]);
        let view = Self::create_view(&texture);
        let sampler = Self::create_sampler(renderer, address_mode, mip_level_count);
        let bind_group_layout = ImageTextureArray::create_bind_group_layout(renderer, None);
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

    fn create_texture(
        Renderer { device, queue, .. }: &Renderer,
        image: &RgbaImage,
        mip_level_count: u32,
        is_srgb: bool,
    ) -> wgpu::Texture {
        let (width, height) = image.dimensions();
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let mut usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
        if mip_level_count > 1 {
            usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: if is_srgb {
                wgpu::TextureFormat::Rgba8UnormSrgb
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            },
            usage,
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

        texture
    }

    fn create_view(texture: &wgpu::Texture) -> wgpu::TextureView {
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
}

pub(crate) struct ImageTextureArray {
    pub(crate) mut(self) bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) mut(self) bind_group: wgpu::BindGroup,
}

#[bon]
impl ImageTextureArray {
    #[builder]
    pub(crate) fn new<'a, R: IntoIterator<Item = &'a RgbaImage>>(
        renderer @ Renderer { device, .. }: &Renderer,
        images: R,
        #[builder(default = 1)] mip_level_count: u32,
        is_srgb: bool,
        #[builder(default)] address_mode: wgpu::AddressMode,
    ) -> Self {
        let textures = images
            .into_iter()
            .map(|image| ImageTexture::create_texture(renderer, image, mip_level_count, is_srgb))
            .collect::<Vec<_>>();
        Self::generate_mips(renderer, &textures);
        let views = textures
            .iter()
            .map(ImageTexture::create_view)
            .collect::<Vec<_>>();
        let sampler = ImageTexture::create_sampler(renderer, address_mode, mip_level_count);
        let bind_group_layout = Self::create_bind_group_layout(renderer, Some(views.len() as u32));
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

    fn generate_mips<'a, T>(renderer @ Renderer { device, queue, .. }: &Renderer, textures: T)
    where
        T: IntoIterator<Item = &'a wgpu::Texture>,
    {
        let mut textures = textures.into_iter().peekable();
        let format = textures.peek().unwrap().format();
        let mip_generator = mip::Generator::new(renderer, format);

        let mut encoder = device.create_command_encoder(&Default::default());
        for texture in textures {
            mip_generator.generate(renderer, &mut encoder, texture);
        }
        queue.submit([encoder.finish()]);
    }

    fn create_bind_group_layout(
        Renderer { device, .. }: &Renderer,
        count: Option<u32>,
    ) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: count.and_then(NonZero::new),
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

pub(crate) fn load_rgba<P: AsRef<Path>>(path: P) -> RgbaImage {
    let path = path.as_ref();
    image::open(path)
        .unwrap_or_else(|e| panic!("failed to open {}: {e}", path.display()))
        .into_rgba8()
}
