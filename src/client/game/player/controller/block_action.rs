use super::Changes;
use crate::client::{
    CLIENT_CONFIG,
    event_loop::{Event, EventHandler},
    game::player::PlayerFeatures,
};
use crate::shared::pacer::Pacer;
use bitflags::{Flags, bitflags};
use std::time::{Duration, Instant};
use winit::event::{ButtonSource, ElementState, MouseButton, WindowEvent};

pub struct BlockActionController {
    relevant: MouseButtons,
    history: MouseButtons,
    pacer: Pacer,
}

impl BlockActionController {
    #[rustfmt::skip]
    pub fn fire(&mut self, now: Instant) -> Option<Changes> {
        let action = if self.relevant.contains(MouseButtons::RIGHT) {
            Changes::BLOCK_PLACED
        } else if self.relevant.contains(MouseButtons::LEFT) {
            Changes::BLOCK_DESTROYED
        } else {
            return None;
        };

        if CLIENT_CONFIG.player.features.contains(PlayerFeatures::DRAWING_MODE) {
            return self.pacer.admit(now).then_some(action);
        }

        self.relevant.clear();
        self.history.clear();
        Some(action)
    }

    fn press(&mut self, button: MouseButtons, opp: MouseButtons) {
        self.relevant.insert(button);
        self.relevant.remove(opp);
        self.history.insert(button);
    }

    fn release(&mut self, button: MouseButtons, opp: MouseButtons) {
        self.relevant.remove(button);
        if self.history.contains(opp) {
            self.relevant.insert(opp);
        }
        self.history.remove(button);
        self.pacer.clear();
    }
}

impl Default for BlockActionController {
    fn default() -> Self {
        Self {
            relevant: Default::default(),
            history: Default::default(),
            pacer: Pacer::new(BLOCK_ACTION_REPEAT_GAP),
        }
    }
}

impl EventHandler for BlockActionController {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        if let Event::WindowEvent(WindowEvent::PointerButton {
            button: ButtonSource::Mouse(button),
            state,
            ..
        }) = event
        {
            let (button, opp) = match button {
                MouseButton::Left => (MouseButtons::LEFT, MouseButtons::RIGHT),
                MouseButton::Right => (MouseButtons::RIGHT, MouseButtons::LEFT),
                _ => return,
            };

            match state {
                ElementState::Pressed => {
                    self.press(button, opp);
                }
                ElementState::Released => {
                    self.release(button, opp);
                }
            }
        }
    }
}

bitflags! {
    #[derive(Clone, Copy, Default)]
    struct MouseButtons: u8 {
        const LEFT = 1 << 0;
        const RIGHT = 1 << 1;
    }
}

const BLOCK_ACTION_REPEAT_GAP: Duration = Duration::from_millis(8);
