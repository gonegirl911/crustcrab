use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{program::Program, uniform::Uniform, Renderer},
    },
    server::game::world::block::Block,
};
use bytemuck::{Pod, Zeroable};
use nalgebra::Matrix4;
use winit::event::{ElementState, KeyboardInput, VirtualKeyCode, WindowEvent};

pub struct Inventory {
    uniform: Uniform<InventoryUniformData>,
    program: Program,
    inventory: [Option<Block>; 9],
    selected_block: Option<Block>,
    is_block_selection_updated: bool,
}

impl Inventory {
    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.program.bind(render_pass, [self.uniform.bind_group()]);
        render_pass.draw(0..Block::MAX_VERTICES_COUNT as u32, 0..1);
    }

    pub fn selected_block(&self) -> Option<Block> {
        self.selected_block
    }

    fn block(&self, keycode: VirtualKeyCode) -> Result<Option<Block>, ()> {
        match keycode {
            VirtualKeyCode::Key1 => Ok(self.inventory[0]),
            VirtualKeyCode::Key2 => Ok(self.inventory[1]),
            VirtualKeyCode::Key3 => Ok(self.inventory[2]),
            VirtualKeyCode::Key4 => Ok(self.inventory[3]),
            VirtualKeyCode::Key5 => Ok(self.inventory[4]),
            VirtualKeyCode::Key6 => Ok(self.inventory[5]),
            VirtualKeyCode::Key7 => Ok(self.inventory[6]),
            VirtualKeyCode::Key8 => Ok(self.inventory[7]),
            VirtualKeyCode::Key9 => Ok(self.inventory[8]),
            _ => Err(()),
        }
    }
}

impl EventHandler for Inventory {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer @ Renderer { config, .. }: Self::Context<'_>) {
        match event {
            Event::WindowEvent {
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
            } => {
                if let Ok(block) = self.block(*keycode) {
                    if self.selected_block != block {
                        self.selected_block = block;
                        self.is_block_selection_updated = true;
                    }
                }
            }
            Event::RedrawRequested(_) => {
                if self.is_block_selection_updated {
                    todo!()
                }
            }
            Event::RedrawEventsCleared => {
                self.is_block_selection_updated = false;
            }
            _ => {}
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct InventoryUniformData {
    transform: Matrix4<f32>,
}

impl InventoryUniformData {
    fn new(transform: Matrix4<f32>) -> Self {
        Self { transform }
    }
}
