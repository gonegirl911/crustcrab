pub mod crosshair;
pub mod inventory;

use self::{crosshair::Crosshair, inventory::Inventory};
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{
            effect::{Blit, Effect, PostProcessor},
            Renderer,
        },
    },
    server::game::world::{block::Block, chunk::Chunk},
};
use nalgebra::{vector, Matrix4, Point3};
use serde::Deserialize;
use std::fs;

pub struct Gui {
    blit: Blit,
    crosshair: Crosshair,
    inventory: Inventory,
    state: ClientState,
}

impl Gui {
    pub fn new(renderer: &Renderer, input_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let blit = Blit::new(renderer, input_bind_group_layout, PostProcessor::FORMAT);
        let crosshair = Crosshair::new(renderer, input_bind_group_layout);
        let state = ClientState::new();
        let inventory = Inventory::new(renderer, state.inventory);
        Self {
            blit,
            crosshair,
            inventory,
            state,
        }
    }

    pub fn selected_block(&self) -> Option<Block> {
        self.inventory.selected_block()
    }

    pub fn render_distance(&self) -> u32 {
        self.state.render_distance
    }

    pub fn origin(&self) -> Point3<f32> {
        self.state.origin
    }

    pub fn fovy(&self) -> f32 {
        self.state.fovy
    }

    pub fn zfar(&self) -> f32 {
        ((self.render_distance() + 1) * Chunk::DIM as u32) as f32
    }

    pub fn speed(&self) -> f32 {
        self.state.speed
    }

    pub fn sensitivity(&self) -> f32 {
        self.state.sensitivity
    }

    fn element_scaling(config: &wgpu::SurfaceConfiguration) -> Matrix4<f32> {
        let size = (config.height as f32 * 0.065).max(27.0);
        Matrix4::new_nonuniform_scaling(&vector![
            size / config.width as f32,
            size / config.height as f32,
            1.0
        ])
    }
}

impl Effect for Gui {
    fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        input_bind_group: &'a wgpu::BindGroup,
    ) {
        self.blit.draw(render_pass, input_bind_group);
        self.crosshair.draw(render_pass, input_bind_group);
        self.inventory.draw(render_pass);
    }
}

impl EventHandler for Gui {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.crosshair.handle(event, renderer);
        self.inventory.handle(event, renderer);
    }
}

#[derive(Deserialize)]
struct ClientState {
    inventory: [Option<Block>; 9],
    render_distance: u32,
    origin: Point3<f32>,
    fovy: f32,
    speed: f32,
    sensitivity: f32,
}

impl ClientState {
    fn new() -> Self {
        toml::from_str(&fs::read_to_string("assets/client.toml").expect("file should exist"))
            .expect("file should be valid")
    }
}
