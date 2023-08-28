use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{
            buffer::Buffer, effect::PostProcessor, mesh::Vertex, program::Program,
            texture::image::ImageTextureArray, uniform::Uniform, Renderer,
        },
    },
    server::{
        game::clock::{Stage, Time},
        ServerEvent,
    },
    shared::{color::Rgb, utils},
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{point, vector, Matrix4, Point3, UnitQuaternion, Vector3};
use once_cell::sync::Lazy;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::Deserialize;
use std::{
    f32::consts::{FRAC_PI_2, PI},
    fs, mem,
};

pub struct Sky {
    objects: Objects,
    uniform: Uniform<SkyUniformData>,
    updated_time: Option<Time>,
}

impl Sky {
    pub fn new(renderer: &Renderer, player_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let uniform = Uniform::uninit_mut(renderer, wgpu::ShaderStages::VERTEX_FRAGMENT);
        let objects = Objects::new(
            renderer,
            player_bind_group_layout,
            uniform.bind_group_layout(),
        );
        Self {
            objects,
            uniform,
            updated_time: Some(Default::default()),
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.uniform.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.uniform.bind_group()
    }

    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_bind_group: &wgpu::BindGroup,
    ) {
        self.objects.draw(
            &mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(Default::default()),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            }),
            player_bind_group,
            self.uniform.bind_group(),
        );
    }
}

impl EventHandler for Sky {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.objects.handle(event, renderer);

        match event {
            Event::UserEvent(ServerEvent::TimeUpdated(time)) => {
                self.updated_time = Some(*time);
            }
            Event::MainEventsCleared => {
                if let Some(time) = self.updated_time.take() {
                    self.uniform
                        .set(renderer, &SKY_STATE.sky_data(time.stage()));
                }
            }
            _ => {}
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct SkyUniformData {
    light_intensity: Rgb<f32>,
    sun_intensity: f32,
}

impl SkyUniformData {
    fn new(light_intensity: Rgb<f32>, sun_intensity: f32) -> Self {
        Self {
            light_intensity,
            sun_intensity,
        }
    }
}

struct Objects {
    stars: StarDome,
    textures: ImageTextureArray,
    program: Program,
    time: Time,
}

impl Objects {
    fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let stars = StarDome::new(renderer, player_bind_group_layout, sky_bind_group_layout);
        let textures = ImageTextureArray::new(
            renderer,
            [
                "assets/textures/sky/sun.png",
                "assets/textures/sky/moon.png",
            ],
            true,
            true,
            1,
        );
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../assets/shaders/object.wgsl"),
            &[],
            &[
                player_bind_group_layout,
                sky_bind_group_layout,
                textures.bind_group_layout(),
            ],
            &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::VERTEX_FRAGMENT,
                range: 0..mem::size_of::<ObjectsPushConstants>() as u32,
            }],
            PostProcessor::FORMAT,
            None,
            None,
            None,
        );
        Self {
            stars,
            textures,
            program,
            time: Default::default(),
        }
    }

    #[rustfmt::skip]
    fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        player_bind_group: &'a wgpu::BindGroup,
        sky_bind_group: &'a wgpu::BindGroup,
    ) {
        self.stars.draw(render_pass, player_bind_group, sky_bind_group);
        self.program.bind(
            render_pass,
            [
                player_bind_group,
                sky_bind_group,
                self.textures.bind_group(),
            ],
        );
        render_pass.set_push_constants(
            wgpu::ShaderStages::VERTEX_FRAGMENT,
            0,
            bytemuck::cast_slice(&[ObjectsPushConstants::new_sun(self.time)]),
        );
        render_pass.draw(0..6, 0..1);
        render_pass.set_push_constants(
            wgpu::ShaderStages::VERTEX_FRAGMENT,
            0,
            bytemuck::cast_slice(&[ObjectsPushConstants::new_moon(self.time)]),
        );
        render_pass.draw(0..6, 0..1);
    }
}

impl EventHandler for Objects {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.stars.handle(event, renderer);

        if let Event::UserEvent(ServerEvent::TimeUpdated(time)) = event {
            self.time = *time;
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct ObjectsPushConstants {
    m: Matrix4<f32>,
    tex_idx: u32,
}

impl ObjectsPushConstants {
    fn new_sun(time: Time) -> Self {
        Self::new(
            time.earth_rotation() * Vector3::x(),
            SKY_STATE.sun_size,
            0,
            time.is_am(),
        )
    }

    fn new_moon(time: Time) -> Self {
        Self::new(
            time.earth_rotation() * -Vector3::x(),
            SKY_STATE.moon_size,
            1,
            time.is_am(),
        )
    }

    fn new(dir: Vector3<f32>, size: f32, tex_idx: u32, is_am: bool) -> Self {
        Self {
            m: Matrix4::face_towards(&dir.into(), &Point3::origin(), &Self::up(is_am))
                .prepend_nonuniform_scaling(&vector![size, size, 1.0]),
            tex_idx,
        }
    }

    fn up(is_am: bool) -> Vector3<f32> {
        if is_am {
            -Vector3::y()
        } else {
            Vector3::y()
        }
    }
}

struct StarDome {
    stars: Vec<Star>,
    instance_buffer: Buffer<[StarInstance]>,
    program: Program,
    updated_rotation: Option<UnitQuaternion<f32>>,
}

impl StarDome {
    fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let count = SKY_STATE.star_count;
        let stars = {
            let mut rng = StdRng::seed_from_u64(808);
            (0..count).map(|_| Star::new(&mut rng)).collect()
        };
        let instance_buffer = Buffer::<[_]>::new(
            renderer,
            Err(count),
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../assets/shaders/star.wgsl"),
            &[StarInstance::desc()],
            &[player_bind_group_layout, sky_bind_group_layout],
            &[],
            PostProcessor::FORMAT,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            None,
            None,
        );
        Self {
            stars,
            instance_buffer,
            program,
            updated_rotation: Some(Default::default()),
        }
    }

    #[rustfmt::skip]
    fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        player_bind_group: &'a wgpu::BindGroup,
        sky_bind_group: &'a wgpu::BindGroup,
    ) {
        self.program.bind(render_pass, [player_bind_group, sky_bind_group]);
        render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        render_pass.draw(0..6, 0..self.instance_buffer.len());
    }
}

impl EventHandler for StarDome {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        match event {
            Event::UserEvent(ServerEvent::TimeUpdated(time)) => {
                self.updated_rotation = Some(time.earth_rotation());
            }
            Event::MainEventsCleared => {
                if let Some(rotation) = self.updated_rotation {
                    self.instance_buffer.write(
                        renderer,
                        &self
                            .stars
                            .iter()
                            .copied()
                            .map(|star| StarInstance::new(star, rotation))
                            .collect::<Vec<_>>(),
                    );
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy)]
struct Star {
    coords: Point3<f32>,
    rotation: f32,
}

impl Star {
    fn new<R: Rng>(rng: &mut R) -> Self {
        Self {
            coords: Self::spherical_coords(rng),
            rotation: rng.gen_range(0.0..FRAC_PI_2),
        }
    }

    fn spherical_coords<R: Rng>(rng: &mut R) -> Point3<f32> {
        let theta = rng.gen_range(-PI..=PI);
        let phi = rng.gen_range(-1.0f32..=1.0).acos();
        point![theta.cos() * phi.sin(), phi.cos(), theta.sin() * phi.sin()]
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct StarInstance {
    m: Matrix4<f32>,
}

impl StarInstance {
    fn new(Star { coords, rotation }: Star, earth_rotation: UnitQuaternion<f32>) -> Self {
        let size = SKY_STATE.star_size;
        Self {
            m: Matrix4::new_rotation(Vector3::z() * rotation)
                * Matrix4::face_towards(
                    &(earth_rotation * coords),
                    &Point3::origin(),
                    &Vector3::y(),
                )
                .prepend_nonuniform_scaling(&vector![size, size, 1.0]),
        }
    }
}

impl Vertex for StarInstance {
    const ATTRIBS: &'static [wgpu::VertexAttribute] =
        &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4];
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
}

#[derive(Deserialize)]
struct SkyState {
    day_intensity: Rgb<f32>,
    night_intensity: Rgb<f32>,
    sun_intensity: f32,
    sun_size: f32,
    moon_size: f32,
    star_size: f32,
    star_count: usize,
}

impl SkyState {
    fn sky_data(&self, stage: Stage) -> SkyUniformData {
        match stage {
            Stage::Dawn { progress } => SkyUniformData::new(
                utils::lerp(self.night_intensity, self.day_intensity, progress),
                utils::lerp(0.0, self.sun_intensity, progress),
            ),
            Stage::Day => SkyUniformData::new(self.day_intensity, self.sun_intensity),
            Stage::Dusk { progress } => SkyUniformData::new(
                utils::lerp(self.day_intensity, self.night_intensity, progress),
                utils::lerp(self.sun_intensity, 0.0, progress),
            ),
            Stage::Night => SkyUniformData::new(self.night_intensity, 0.0),
        }
    }
}

static SKY_STATE: Lazy<SkyState> = Lazy::new(|| {
    toml::from_str(&fs::read_to_string("assets/sky.toml").expect("file should exist"))
        .expect("file should be valid")
});
