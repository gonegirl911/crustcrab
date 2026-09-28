use super::{clock::RenderTime, world::BlockVertex};
use crate::{
    client::{
        CLIENT_CONFIG,
        renderer::{
            Renderer, Surface,
            buffer::{MemoryState, VertexBuffer},
            effect::{Blender, PostProcessor},
            render_pipeline::RenderPipeline,
            texture::{image::ImageTexture, screen::DepthBuffer},
            utils::{Immediates, Vertex, color_pass, load_rgba, read_wgsl},
        },
    },
    server::game::world::{block::Block, chunk::Chunk},
    shared::{
        color::{Float3, Rgb},
        utils,
    },
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{Point2, Point3, Vector2, point, vector};
use serde::Deserialize;

pub struct CloudLayer {
    vertex_buffer: VertexBuffer<BlockVertex>,
    instance_buffer: VertexBuffer<CloudInstance>,
    texture: ImageTexture,
    render_pipeline: RenderPipeline,
    blender: Blender,
    tex_dims: (u32, u32),
}

impl CloudLayer {
    pub fn new(
        renderer: &Renderer,
        surface: &Surface,
        player_uniform_bind_group_layout: &wgpu::BindGroupLayout,
        shading_uniform_bind_group_layout: &wgpu::BindGroupLayout,
        spare_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let vertex_buffer = VertexBuffer::new(
            renderer,
            MemoryState::Immutable(&Self::vertices().collect::<Vec<_>>()),
        );
        let instance_buffer = VertexBuffer::new(
            renderer,
            MemoryState::Immutable(&Self::instances().collect::<Vec<_>>()),
        );
        let image = load_rgba("assets/textures/clouds.png");
        let texture = ImageTexture::builder()
            .renderer(renderer)
            .surface(surface)
            .image(&image)
            .is_srgb(false)
            .address_mode(wgpu::AddressMode::Repeat)
            .build();
        let render_pipeline = RenderPipeline::builder()
            .renderer(renderer)
            .shader_desc(read_wgsl("assets/shaders/cloud.wgsl"))
            .bind_group_layouts(&[
                player_uniform_bind_group_layout,
                shading_uniform_bind_group_layout,
                &texture.bind_group_layout,
            ])
            .immediate_size(CloudImmediates::SIZE)
            .buffers(&[BlockVertex::desc(), CloudInstance::desc()])
            .depth_stencil(wgpu::DepthStencilState {
                format: DepthBuffer::FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            })
            .format(PostProcessor::FORMAT)
            .build();
        let blender = Blender::new(renderer, spare_bind_group_layout, PostProcessor::FORMAT);
        Self {
            vertex_buffer,
            instance_buffer,
            texture,
            render_pipeline,
            blender,
            tex_dims: image.dimensions(),
        }
    }

    #[expect(clippy::too_many_arguments)]
    #[rustfmt::skip]
    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        spare_view: &wgpu::TextureView,
        player_uniform_bind_group: &wgpu::BindGroup,
        shading_uniform_bind_group: &wgpu::BindGroup,
        depth_view: &wgpu::TextureView,
        spare_bind_group: &wgpu::BindGroup,
        origin: Point3<f64>,
        time: RenderTime,
    ) {
        let nightness = time.nightness();
        let scroll_x = -CLIENT_CONFIG.cloud.drift_per_tick as f64 * time.ticks;
        let imm = CloudImmediates::new(self.tex_dims, origin, scroll_x, nightness);
        let opacity = CLIENT_CONFIG.cloud.opacity(nightness);

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: spare_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(Default::default()),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            self.render_pipeline.bind(
                &mut render_pass,
                [
                    player_uniform_bind_group,
                    shading_uniform_bind_group,
                    &self.texture.bind_group,
                ],
            );
            imm.set(&mut render_pass);
            self.vertex_buffer.draw_instanced(&mut render_pass, &self.instance_buffer);
        }
        self.blender.draw(
            &mut color_pass(view, encoder, wgpu::LoadOp::Clear(Default::default())),
            spare_bind_group,
            opacity,
        );
    }

    fn vertices() -> impl Iterator<Item = BlockVertex> {
        Block::SAND.data().isolated_mesh()
    }

    fn instances() -> impl Iterator<Item = CloudInstance> {
        let render_distance = CLIENT_CONFIG.player.render_distance as u64 * Chunk::DIM as u64;
        let radius = (render_distance / CLIENT_CONFIG.cloud.size.x) as i32;
        (-radius..=radius)
            .flat_map(move |dx| (-radius..=radius).map(move |dz| vector![dx, dz]))
            .filter(move |&offset| utils::magnitude_squared(offset) <= (radius as u128).pow(2))
            .map(CloudInstance::new)
    }
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct CloudInstance {
    offset: Vector2<f32>,
}

impl CloudInstance {
    fn new(offset: Vector2<i32>) -> Self {
        Self {
            offset: (offset.cast() * CLIENT_CONFIG.cloud.size.x as i64).cast(),
        }
    }
}

impl Vertex for CloudInstance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBS: &[wgpu::VertexAttribute] = &wgpu::vertex_attr_array![1 => Float32x2];
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct CloudImmediates {
    tex_dims: Point2<f32>,
    size: Point2<f32>,
    scale_factor: Float3,
    color: Float3,
    phase: Vector2<f32>,
    altitude: f32,
    padding: f32,
}

impl CloudImmediates {
    fn new(
        (tex_width, tex_height): (u32, u32),
        origin: Point3<f64>,
        scroll_x: f64,
        nightness: f32,
    ) -> Self {
        let config = &CLIENT_CONFIG.cloud;
        let tex_dims = point![tex_width, tex_height];
        let size = config.size;
        let scale_factor = size.map(|c| 1.0 + config.padding * 2.0 / c as f32);
        let period = size.x as f64 * tex_dims.x as f64;
        let camera_xz = origin.xz().coords - vector![scroll_x, 0.0];
        let phase = camera_xz.map(|c| c.rem_euclid(period));
        let altitude = (config.altitude - origin.y) as f32;
        Self {
            tex_dims: tex_dims.cast(),
            size: size.cast(),
            scale_factor: scale_factor.xyx().into(),
            color: config.color(nightness).into(),
            phase: phase.cast(),
            altitude,
            padding: Default::default(),
        }
    }
}

impl Immediates for CloudImmediates {}

#[derive(Deserialize)]
pub struct CloudConfig {
    size: Point2<u64>,
    pub padding: f32,
    drift_per_tick: f32,
    altitude: f64,
    day: TimePhaseConfig,
    night: TimePhaseConfig,
}

impl CloudConfig {
    fn color(&self, nightness: f32) -> Rgb<f32> {
        utils::lerp(self.day.color, self.night.color, nightness)
    }

    fn opacity(&self, nightness: f32) -> f32 {
        utils::lerp(self.day.opacity, self.night.opacity, nightness)
    }
}

#[derive(Deserialize)]
struct TimePhaseConfig {
    color: Rgb<f32>,
    opacity: f32,
}
