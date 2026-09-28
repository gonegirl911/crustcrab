use crate::client::renderer::Renderer;
use bon::bon;
use bytemuck::Pod;

pub struct DataTexture {
    texture: wgpu::Texture,
    pub mut(self) bind_group_layout: wgpu::BindGroupLayout,
    pub mut(self) bind_group: wgpu::BindGroup,
    size: wgpu::Extent3d,
    format: wgpu::TextureFormat,
}

#[bon]
impl DataTexture {
    #[builder]
    pub fn new(
        _renderer @ Renderer { device, .. }: &Renderer,
        size: wgpu::Extent3d,
        dimension: wgpu::TextureDimension,
        format: wgpu::TextureFormat,
        visibility: wgpu::ShaderStages,
        filterable: bool,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let filter = if filterable {
            wgpu::FilterMode::Linear
        } else {
            wgpu::FilterMode::Nearest
        };
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: filter,
            min_filter: filter,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
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
            size,
            format,
        }
    }

    pub fn write<T: Pod>(&self, Renderer { queue, .. }: &Renderer, data: &[T]) {
        let block_size = self.format.block_copy_size(None).unwrap();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(data),
            wgpu::TexelCopyBufferLayout {
                bytes_per_row: Some(block_size * self.size.width),
                rows_per_image: Some(self.size.height),
                ..Default::default()
            },
            self.size,
        );
    }
}
