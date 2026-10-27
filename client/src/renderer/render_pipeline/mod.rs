pub(crate) mod blender;

use super::Renderer;
use bon::bon;
use std::{fs, path::Path};

pub(crate) struct RenderPipeline(wgpu::RenderPipeline);

#[bon]
impl RenderPipeline {
    #[builder]
    pub(crate) fn new<'a>(
        _renderer @ Renderer { device, .. }: &'a Renderer,
        shader_desc: wgpu::ShaderModuleDescriptor<'a>,
        #[builder(default)] bind_group_layouts: &'a [Option<&'a wgpu::BindGroupLayout>],
        #[builder(default)] immediate_size: u32,
        #[builder(default)] buffers: &'a [Option<wgpu::VertexBufferLayout<'a>>],
        cull_mode: Option<wgpu::Face>,
        depth_stencil: Option<wgpu::DepthStencilState>,
        fragment_entry_point: Option<&'a str>,
        format: wgpu::TextureFormat,
        blend: Option<wgpu::BlendState>,
    ) -> Self {
        let shader = device.create_shader_module(shader_desc);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts,
            immediate_size,
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                buffers,
            },
            primitive: wgpu::PrimitiveState {
                cull_mode,
                ..Default::default()
            },
            depth_stencil,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: fragment_entry_point,
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self(render_pipeline)
    }

    pub(crate) fn bind<'a, I>(&self, render_pass: &mut wgpu::RenderPass, bind_groups: I)
    where
        I: IntoIterator<Item = &'a wgpu::BindGroup>,
    {
        render_pass.set_pipeline(&self.0);
        for (i, bind_group) in (0..).zip(bind_groups) {
            render_pass.set_bind_group(i, bind_group, &[]);
        }
    }

    pub(crate) fn get_bind_group_layout(&self, index: u32) -> wgpu::BindGroupLayout {
        self.0.get_bind_group_layout(index)
    }
}

pub(crate) fn read_wgsl<P: AsRef<Path>>(path: P) -> wgpu::ShaderModuleDescriptor<'static> {
    let path = path.as_ref();
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(contents.into()),
    }
}
