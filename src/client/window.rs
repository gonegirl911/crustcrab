use super::event_loop::{Event, EventHandler};
use std::{ops::Deref, sync::Arc};
use winit::{
    event::{ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::EventLoop as RawEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window as RawWindow, WindowBuilder as RawWindowBuilder},
};

pub struct Window(Arc<RawWindow>);

impl Window {
    pub fn new<T>(event_loop: &RawEventLoop<T>) -> Self {
        Self(
            RawWindowBuilder::new()
                .with_title("Crustcrab")
                .build(event_loop)
                .expect("window should be buildable")
                .into(),
        )
    }

    fn set_cursor_grab<I: IntoIterator<Item = CursorGrabMode>>(&self, modes: I) {
        modes
            .into_iter()
            .map(|mode| self.0.set_cursor_grab(mode).err())
            .collect::<Option<Vec<_>>>()
            .map_or(Ok(()), Err)
            .expect("cursor should be grabbable");
    }
}

impl EventHandler for Window {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::MouseInput {
                    button: MouseButton::Left,
                    state: ElementState::Pressed,
                    ..
                } => {
                    self.set_cursor_grab([CursorGrabMode::Confined, CursorGrabMode::Locked]);
                    self.0.set_cursor_visible(false);
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
                    self.set_cursor_grab([CursorGrabMode::None]);
                    self.0.set_cursor_visible(true);
                }
                _ => {}
            },
            Event::AboutToWait => self.0.request_redraw(),
            _ => {}
        }
    }
}

impl Deref for Window {
    type Target = Arc<RawWindow>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
