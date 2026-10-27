use crate::renderer::{Renderer, post_process::blit::Blit, render_pass::color_pass};

pub(super) struct Generator {
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    blit: Blit,
}

impl Generator {
    pub(super) fn new(
        renderer @ Renderer { device, .. }: &Renderer,
        format: wgpu::TextureFormat,
    ) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
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
        let blit = Blit::builder()
            .renderer(renderer)
            .input_bind_group_layout(&bind_group_layout)
            .format(format)
            .build();
        Self {
            sampler,
            bind_group_layout,
            blit,
        }
    }

    pub(super) fn generate(
        &self,
        Renderer { device, .. }: &Renderer,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
    ) {
        let mut views = (0..texture.mip_level_count()).map(|level| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: level,
                mip_level_count: Some(1),
                ..Default::default()
            })
        });
        let mut src = views.next().unwrap();

        for dst in views {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&src),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.blit.draw(
                &mut color_pass(&dst, encoder, wgpu::LoadOp::Clear(Default::default())),
                &bind_group,
            );
            src = dst;
        }
    }
}
