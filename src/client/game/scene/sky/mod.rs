pub mod color_map;
pub mod sphere;

use self::{
    color_map::ColorMap,
    sphere::{Sphere, SphereMesh, SphereVertex},
};
use super::depth_buffer::DepthBuffer;
use crate::client::renderer::Renderer;

pub struct Sky {
    mesh: SphereMesh,
    color_map: ColorMap,
    render_pipeline: wgpu::RenderPipeline,
}

impl Sky {
    pub fn new(
        renderer @ Renderer { device, config, .. }: &Renderer,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        clock_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let sphere = Sphere::new(32, 16);
        let mesh = SphereMesh::new(
            renderer,
            &sphere.vertices().collect::<Vec<_>>(),
            &sphere.indices().collect::<Vec<_>>(),
        );
        let color_map = ColorMap::new(renderer);
        let shader = device.create_shader_module(wgpu::include_wgsl!(
            "../../../../../assets/shaders/sky.wgsl"
        ));
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[
                    camera_bind_group_layout,
                    clock_bind_group_layout,
                    color_map.bind_group_layout(),
                ],
                push_constant_ranges: &[],
            });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[SphereVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Front),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthBuffer::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
        });
        Self {
            mesh,
            color_map,
            render_pipeline,
        }
    }

    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
        clock_bind_group: &'a wgpu::BindGroup,
    ) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_bind_group(1, clock_bind_group, &[]);
        render_pass.set_bind_group(2, self.color_map.bind_group(), &[]);
        self.mesh.draw(render_pass);
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.color_map.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.color_map.bind_group()
    }
}
