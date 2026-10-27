use super::event::Event;
use crate::event;
use log::warn;
use std::{num::NonZero, sync::Arc};
use winit::{
    dpi::PhysicalSize,
    error::RequestError,
    event::{ButtonSource, ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, WindowAttributes},
};

pub(super) struct Window {
    pub(super) mut(self) raw: Arc<dyn winit::window::Window>,
    is_occluded: bool,
}

impl Window {
    pub(super) fn new(event_loop: &dyn ActiveEventLoop) -> Self {
        Self {
            raw: event_loop
                .create_window(WindowAttributes::default().with_title("Crustcrab"))
                .expect("window should be creatable")
                .into(),
            is_occluded: false,
        }
    }

    fn grab_cursor(&self) -> Result<(), RequestError> {
        self.raw
            .set_cursor_grab(CursorGrabMode::Confined)
            .or_else(|_| self.raw.set_cursor_grab(CursorGrabMode::Locked))?;
        self.raw.set_cursor_visible(false);
        Ok(())
    }

    fn ungrab_cursor(&self) {
        self.raw.set_cursor_grab(CursorGrabMode::None).unwrap();
        self.raw.set_cursor_visible(true);
    }

    pub(super) fn is_visible(&self) -> bool {
        let PhysicalSize { width, height } = self.raw.surface_size();
        !self.is_occluded && width != 0 && height != 0
    }

    pub(super) fn refresh_rate_millihertz(&self) -> Option<NonZero<u32>> {
        self.raw
            .current_monitor()?
            .current_video_mode()?
            .refresh_rate_millihertz()
    }
}

impl event::Handler for Window {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        if let Event::WindowEvent(event) = event {
            match event {
                WindowEvent::PointerButton {
                    button: ButtonSource::Mouse(MouseButton::Left),
                    state: ElementState::Pressed,
                    ..
                } => {
                    if let Err(e) = self.grab_cursor() {
                        warn!("cursor grab failed: {e}");
                    }
                }
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            physical_key: PhysicalKey::Code(KeyCode::Escape),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    self.ungrab_cursor();
                }
                WindowEvent::Occluded(occluded) => {
                    self.is_occluded = *occluded;
                }
                _ => {}
            }
        }
    }
}
