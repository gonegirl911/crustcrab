use super::{
    clock::RenderTime,
    player::RenderCamera,
    world::{
        block::geometry::{Corner, LL_UR_TRIANGULATION, SIDE_CORNER_DELTAS, SIDE_DELTAS, Side},
        mesh::SideShade,
    },
};
use crate::{
    CONFIG,
    renderer::{
        self, Immediates as _, Renderer, Vertex as _,
        buffer::{MemoryState, index::IndexBuffer, vertex::VertexBuffer},
        post_process::PostProcessor,
        render_pass::color_pass,
        render_pipeline::{RenderPipeline, blender::Blender, read_wgsl},
        texture::{
            data::{DataTexture, R8Unorm},
            image::load_rgba,
            screen::depth::DepthTexture,
        },
        types::Float3,
    },
};
use base::{
    color::Rgb,
    math::{distance::magnitude_squared, interpolation::lerp},
    position::Position3,
};
use bytemuck::{Pod, Zeroable};
use domain::game::world::chunk::Chunk;
use enum_map::Enum;
use image::Pixel;
use nalgebra::{Point2, Point3, Vector2, point, vector};
use serde::Deserialize;

pub(super) struct Layer {
    vertex_buffer: VertexBuffer<Vertex>,
    index_buffer: IndexBuffer<u16>,
    instance_buffer: VertexBuffer<Instance>,
    occupancy: DataTexture<R8Unorm>,
    render_pipeline: RenderPipeline,
    blender: Blender,
}

impl Layer {
    pub(super) fn new(
        renderer: &Renderer,
        player_uniform_bind_group_layout: &wgpu::BindGroupLayout,
        shading_uniform_bind_group_layout: &wgpu::BindGroupLayout,
        spare_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let vertex_buffer = VertexBuffer::new(
            renderer,
            MemoryState::Immutable(&Self::vertices().collect::<Vec<_>>()),
        );
        let index_buffer = IndexBuffer::new(
            renderer,
            MemoryState::Immutable(&Self::indices().collect::<Vec<_>>()),
        );
        let instance_buffer = VertexBuffer::new(
            renderer,
            MemoryState::Immutable(&Self::instances().collect::<Vec<_>>()),
        );
        let (occupancy_width, occupancy_height, occupancy_data) = Self::occupancy();
        let occupancy = DataTexture::builder()
            .renderer(renderer)
            .size(wgpu::Extent3d {
                width: occupancy_width,
                height: occupancy_height,
                depth_or_array_layers: 1,
            })
            .dimension(wgpu::TextureDimension::D2)
            .data(&occupancy_data)
            .address_mode(wgpu::AddressMode::Repeat)
            .visibility(wgpu::ShaderStages::VERTEX)
            .build();
        let render_pipeline = RenderPipeline::builder()
            .renderer(renderer)
            .shader_desc(read_wgsl("client/assets/shaders/cloud.wgsl"))
            .bind_group_layouts(&[
                Some(player_uniform_bind_group_layout),
                Some(shading_uniform_bind_group_layout),
                Some(&occupancy.bind_group_layout),
            ])
            .immediate_size(Immediates::SIZE)
            .buffers(&[Some(Vertex::desc()), Some(Instance::desc())])
            .depth_stencil(wgpu::DepthStencilState {
                format: DepthTexture::FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: -1,
                    ..Default::default()
                },
            })
            .format(PostProcessor::FORMAT)
            .build();
        let blender = Blender::new(renderer, spare_bind_group_layout, PostProcessor::FORMAT);
        Self {
            vertex_buffer,
            index_buffer,
            instance_buffer,
            occupancy,
            render_pipeline,
            blender,
        }
    }

    #[expect(clippy::too_many_arguments)]
    pub(super) fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        spare_view: &wgpu::TextureView,
        player_uniform_bind_group: &wgpu::BindGroup,
        shading_uniform_bind_group: &wgpu::BindGroup,
        depth_texture_view: &wgpu::TextureView,
        spare_bind_group: &wgpu::BindGroup,
        time @ &RenderTime { nightness, .. }: &RenderTime,
        &RenderCamera { origin, .. }: &RenderCamera,
    ) {
        let imm = Immediates::new(
            self.occupancy.width(),
            self.occupancy.height(),
            time,
            origin,
        );
        let opacity = CONFIG.cloud.opacity(nightness);

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
                    view: depth_texture_view,
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
                    &self.occupancy.bind_group,
                ],
            );
            imm.set(&mut render_pass);
            self.vertex_buffer.draw_indexed_instanced(
                &mut render_pass,
                &self.index_buffer,
                &self.instance_buffer,
            );
        }
        self.blender.draw(
            &mut color_pass(view, encoder, wgpu::LoadOp::Clear(Default::default())),
            spare_bind_group,
            opacity,
        );
    }

    fn vertices() -> impl Iterator<Item = Vertex> {
        SIDE_CORNER_DELTAS
            .into_iter()
            .flat_map(|(side, corner_deltas)| {
                let side_delta = SIDE_DELTAS[side].xz();
                let side_shade = side.into();
                corner_deltas
                    .into_values()
                    .map(move |delta| Vertex::new(delta.into(), side_delta, side_shade))
            })
    }

    fn indices() -> impl Iterator<Item = u16> {
        Side::variants().flat_map(|side| {
            let base = side.to_index() * Corner::LEN;
            LL_UR_TRIANGULATION.into_iter().map(move |corner| {
                let offset = corner.to_index();
                (base + offset) as u16
            })
        })
    }

    fn instances() -> impl Iterator<Item = Instance> {
        let render_distance = CONFIG.player.render_distance;
        let size_x = CONFIG.cloud.size.x;
        let radius = (render_distance as u64 * Chunk::DIM as u64 / size_x) as i32;
        (-radius..=radius)
            .flat_map(move |dx| (-radius..=radius).map(move |dz| vector![dx, dz]))
            .filter(move |&offset| magnitude_squared(offset) <= (radius as u128).pow(2))
            .map(move |offset| Instance::new(offset.cast() * size_x as i64))
    }

    fn occupancy() -> (u32, u32, Vec<u8>) {
        let image = load_rgba("client/assets/textures/clouds.png");
        let data = image.pixels().map(Pixel::alpha).collect();
        (image.width(), image.height(), data)
    }
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Vertex {
    coords: Point3<f32>,
    side_delta_xz: Vector2<f32>,
    side_shade: u32,
}

impl Vertex {
    fn new(coords: Point3<u8>, side_delta_xz: Vector2<i8>, side_shade: SideShade) -> Self {
        Self {
            coords: coords.cast(),
            side_delta_xz: side_delta_xz.cast(),
            side_shade: side_shade as u32,
        }
    }
}

impl renderer::Vertex for Vertex {
    const ATTRIBS: &[wgpu::VertexAttribute] =
        &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Uint32];
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Instance {
    offset: Vector2<f32>,
}

impl Instance {
    fn new(offset: Vector2<i64>) -> Self {
        Self {
            offset: offset.cast(),
        }
    }
}

impl renderer::Vertex for Instance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBS: &[wgpu::VertexAttribute] = &wgpu::vertex_attr_array![3 => Float32x2];
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Immediates {
    occupancy_dims: Point2<f32>,
    size: Point2<f32>,
    color: Float3,
    phase: Vector2<f32>,
    altitude: f32,
    padding: f32,
}

impl Immediates {
    fn new(
        occupancy_width: u32,
        occupancy_height: u32,
        &RenderTime {
            position: time_position,
            nightness,
            ..
        }: &RenderTime,
        origin: Position3,
    ) -> Self {
        let size = CONFIG.cloud.size;
        let period = size.x as i64 * occupancy_width as i64;
        let scroll_x = -(time_position * CONFIG.cloud.drift_per_tick)
            .rem_euclid(period)
            .displacement_from(0);
        let camera_xz = origin.xz() - vector![scroll_x, 0.0];
        let phase = camera_xz
            .rem_euclid(Vector2::repeat(period))
            .displacement_from(Point2::origin());
        let altitude = -origin.y().displacement_from(CONFIG.cloud.altitude);
        Self {
            occupancy_dims: point![occupancy_width, occupancy_height].cast(),
            size: size.cast(),
            color: CONFIG.cloud.color(nightness).into(),
            phase,
            altitude,
            padding: Default::default(),
        }
    }
}

impl renderer::Immediates for Immediates {}

#[derive(Deserialize)]
pub(crate) struct Config {
    size: Point2<u64>,
    drift_per_tick: f32,
    altitude: i64,
    day: TimePhaseConfig,
    night: TimePhaseConfig,
}

impl Config {
    fn color(&self, nightness: f32) -> Rgb<f32> {
        lerp(self.day.color, self.night.color, nightness)
    }

    fn opacity(&self, nightness: f32) -> f32 {
        lerp(self.day.opacity, self.night.opacity, nightness)
    }
}

#[derive(Deserialize)]
struct TimePhaseConfig {
    color: Rgb<f32>,
    opacity: f32,
}
