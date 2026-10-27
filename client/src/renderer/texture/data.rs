use crate::renderer::Renderer;
use base::color::Rgba;
use bon::bon;
use bytemuck::Pod;
use std::marker::PhantomData;
use wgpu::{util::DeviceExt, wgt::TextureDataOrder};

pub(crate) struct DataTexture<T: Texel> {
    texture: wgpu::Texture,
    pub(crate) mut(self) bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) mut(self) bind_group: wgpu::BindGroup,
    phantom: PhantomData<[T::Repr]>,
}

#[bon]
impl<T: Texel> DataTexture<T> {
    #[builder]
    pub(crate) fn new(
        _renderer @ Renderer { device, queue, .. }: &Renderer,
        size: wgpu::Extent3d,
        dimension: wgpu::TextureDimension,
        data: Option<&[T::Repr]>,
        #[builder(default)] address_mode: wgpu::AddressMode,
        visibility: wgpu::ShaderStages,
        #[builder(default)] filterable: bool,
    ) -> Self {
        let desc = wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension,
            format: T::FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };
        let texture = if let Some(data) = data {
            device.create_texture_with_data(
                queue,
                &desc,
                TextureDataOrder::LayerMajor,
                bytemuck::cast_slice(data),
            )
        } else {
            device.create_texture(&desc)
        };
        let view = texture.create_view(&Default::default());
        let filter = if filterable {
            wgpu::FilterMode::Linear
        } else {
            wgpu::FilterMode::Nearest
        };
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            address_mode_w: address_mode,
            mag_filter: filter,
            min_filter: filter,
            ..Default::default()
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable },
                        view_dimension: match dimension {
                            wgpu::TextureDimension::D1 => wgpu::TextureViewDimension::D1,
                            wgpu::TextureDimension::D2 => wgpu::TextureViewDimension::D2,
                            wgpu::TextureDimension::D3 => wgpu::TextureViewDimension::D3,
                        },
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility,
                    ty: wgpu::BindingType::Sampler(if filterable {
                        wgpu::SamplerBindingType::Filtering
                    } else {
                        wgpu::SamplerBindingType::NonFiltering
                    }),
                    count: None,
                },
            ],
        });
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
            texture,
            bind_group_layout,
            bind_group,
            phantom: PhantomData,
        }
    }

    pub(crate) fn write(&self, Renderer { queue, .. }: &Renderer, data: &[T::Repr]) {
        let block_size = T::FORMAT.block_copy_size(None).unwrap();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(data),
            wgpu::TexelCopyBufferLayout {
                bytes_per_row: Some(block_size * self.width()),
                rows_per_image: Some(self.height()),
                ..Default::default()
            },
            self.texture.size(),
        );
    }

    pub(crate) fn width(&self) -> u32 {
        self.texture.width()
    }

    pub(crate) fn height(&self) -> u32 {
        self.texture.height()
    }
}

pub(crate) impl(self) trait Texel {
    type Repr: Pod;

    const FORMAT: wgpu::TextureFormat;
}

macro_rules! define_texels {
    ($($format:ident => $repr:ty),* $(,)?) => {
        $(
            pub(crate) struct $format;

            impl Texel for $format {
                type Repr = $repr;

                const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::$format;
            }
        )*
    };
}

define_texels! {
    R8Unorm => u8,
    Rgba16Unorm => Rgba<u16>,
}
