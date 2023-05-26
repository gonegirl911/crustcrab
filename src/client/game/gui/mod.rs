pub mod crosshair;

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
use crosshair::Crosshair;
use nalgebra::Point3;
use serde::Deserialize;
use std::fs;
use winit::event::{ElementState, KeyboardInput, VirtualKeyCode, WindowEvent};

pub struct Gui {
    blit: Blit,
    crosshair: Crosshair,
    state: ClientState,
}

impl Gui {
    pub fn new(renderer: &Renderer, input_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        Self {
            blit: Blit::new(renderer, input_bind_group_layout, PostProcessor::FORMAT),
            crosshair: Crosshair::new(renderer, input_bind_group_layout),
            state: ClientState::new(),
        }
    }

    pub fn selected_block(&self) -> Option<Block> {
        self.state.selected_block
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
}

impl Effect for Gui {
    fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        input_bind_group: &'a wgpu::BindGroup,
    ) {
        self.blit.draw(render_pass, input_bind_group);
        self.crosshair.draw(render_pass, input_bind_group);
    }
}

impl EventHandler for Gui {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.crosshair.handle(event, renderer);

        if let Event::WindowEvent {
            event:
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(keycode),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                },
            ..
        } = event
        {
            self.state.selected_block = match keycode {
                VirtualKeyCode::Key1 => Some(Block::Glowstone),
                VirtualKeyCode::Key2 => Some(Block::GlassMagenta),
                VirtualKeyCode::Key3 => Some(Block::GlassCyan),
                _ => self.state.selected_block,
            };
        }
    }
}

#[derive(Deserialize)]
struct ClientState {
    selected_block: Option<Block>,
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
