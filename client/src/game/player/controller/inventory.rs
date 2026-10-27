use crate::event::{self, Event};
use domain::{game::world::block::Block, protocol::control};
use std::sync::Arc;
use winit::{
    event::{ElementState, KeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

#[derive(Default)]
pub(in crate::game::player) struct Controller {
    contents: Arc<[Block]>,
    index: usize,
}

impl Controller {
    pub(in crate::game::player) fn selected_block(&self) -> Option<Block> {
        self.contents.get(self.index).copied()
    }

    fn index(keycode: KeyCode) -> Option<usize> {
        match keycode {
            KeyCode::Digit1 => Some(0),
            KeyCode::Digit2 => Some(1),
            KeyCode::Digit3 => Some(2),
            KeyCode::Digit4 => Some(3),
            KeyCode::Digit5 => Some(4),
            KeyCode::Digit6 => Some(5),
            KeyCode::Digit7 => Some(6),
            KeyCode::Digit8 => Some(7),
            KeyCode::Digit9 => Some(8),
            _ => None,
        }
    }
}

impl event::Handler for Controller {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        match event {
            Event::Control(control::Event::PlayerInitialized { inventory, .. }) => {
                self.contents = inventory.clone();
            }
            &Event::WindowEvent(WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(keycode),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            }) => {
                if let Some(idx) = Self::index(keycode) {
                    self.index = idx;
                }
            }
            _ => {}
        }
    }
}
