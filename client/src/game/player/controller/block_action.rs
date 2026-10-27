use super::Changes;
use crate::event::{self, Event};
use bitflags::{Flags, bitflags};
use winit::event::{ButtonSource, ElementState, MouseButton, WindowEvent};

#[derive(Default)]
pub(super) struct Controller {
    relevant: MouseButtons,
    history: MouseButtons,
}

impl Controller {
    pub(super) fn fire(&mut self) -> Option<Changes> {
        let action = if self.relevant.contains(MouseButtons::RIGHT) {
            Changes::BLOCK_PLACED
        } else if self.relevant.contains(MouseButtons::LEFT) {
            Changes::BLOCK_DESTROYED
        } else {
            return None;
        };
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
    }
}

impl event::Handler for Controller {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        match event {
            Event::WindowEvent(WindowEvent::PointerButton {
                button: ButtonSource::Mouse(button),
                state,
                ..
            }) => {
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
            Event::InputInvalidated => {
                self.relevant = Default::default();
                self.history = Default::default();
            }
            _ => {}
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
