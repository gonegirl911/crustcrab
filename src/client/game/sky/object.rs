use crate::{
    client::{
        CLIENT_CONFIG,
        event_loop::{Event, EventHandler},
        renderer::{
            Renderer,
            effect::PostProcessor,
            program::Program,
            texture::image::ImageTextureArray,
            utils::{Immediates, load_rgba, read_wgsl},
        },
    },
    server::{ServerEvent, game::clock::Time},
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{Matrix4, Point3, Vector3, vector};
use serde::Deserialize;

pub struct ObjectSet {
    textures: ImageTextureArray,
    program: Program,
    sun_imm: ObjectImmediates,
    moon_imm: ObjectImmediates,
}

impl ObjectSet {
    pub fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let textures = ImageTextureArray::builder()
            .renderer(renderer)
            .images([
                load_rgba("assets/textures/sky/sun.png"),
                load_rgba("assets/textures/sky/moon.png"),
            ])
            .is_srgb(true)
            .build();
        let program = Program::builder()
            .renderer(renderer)
            .shader_desc(read_wgsl("assets/shaders/object.wgsl"))
            .bind_group_layouts(&[
                player_bind_group_layout,
                sky_bind_group_layout,
                textures.bind_group_layout(),
            ])
            .immediate_size(ObjectImmediates::SIZE)
            .format(PostProcessor::FORMAT)
            .build();
        let (sun_imm, moon_imm) = Self::imm(Default::default());
        Self {
            textures,
            program,
            sun_imm,
            moon_imm,
        }
    }

    pub fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass,
        player_bind_group: &wgpu::BindGroup,
        sky_bind_group: &wgpu::BindGroup,
    ) {
        self.program.bind(
            render_pass,
            [
                player_bind_group,
                sky_bind_group,
                self.textures.bind_group(),
            ],
        );
        self.sun_imm.set(render_pass);
        render_pass.draw(0..6, 0..1);
        self.moon_imm.set(render_pass);
        render_pass.draw(0..6, 0..1);
    }

    fn imm(time: Time) -> (ObjectImmediates, ObjectImmediates) {
        let sun_dir = time.sky_rotation() * Vector3::x();
        let is_am = time.is_am();
        (
            ObjectImmediates::new(sun_dir, 0, is_am),
            ObjectImmediates::new(-sun_dir, 1, is_am),
        )
    }
}

impl EventHandler for ObjectSet {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        if let Event::ServerEvent(ServerEvent::TimeUpdated(time)) = *event {
            (self.sun_imm, self.moon_imm) = Self::imm(time);
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct ObjectImmediates {
    m: Matrix4<f32>,
    tex_index: u32,
}

impl ObjectImmediates {
    fn new(dir: Vector3<f32>, tex_index: u32, is_am: bool) -> Self {
        let size = CLIENT_CONFIG.sky.object.size;
        Self {
            m: Matrix4::face_towards(&dir.into(), &Point3::origin(), &Self::up(is_am))
                .prepend_nonuniform_scaling(&vector![size, size, 1.0]),
            tex_index,
        }
    }

    fn up(is_am: bool) -> Vector3<f32> {
        if is_am { -Vector3::y() } else { Vector3::y() }
    }
}

impl Immediates for ObjectImmediates {}

#[derive(Deserialize)]
pub struct ObjectConfig {
    size: f32,
}
