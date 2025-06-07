use crate::client::event_loop::{Event, EventHandler};
use std::{ops::Deref, sync::Arc};
use winit::{
    error::ExternalError,
    event::{ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, WindowAttributes},
};

pub use winit::window::Window as RawWindow;

#[derive(Clone)]
pub struct Window(Arc<RawWindow>);

impl Window {
    pub fn new(event_loop: &ActiveEventLoop) -> Self {
        Self(
            event_loop
                .create_window(WindowAttributes::default().with_title("Crustcrab"))
                .expect("window should be creatable")
                .into(),
        )
    }

    fn set_cursor_grab<M>(&self, modes: M) -> Result<(), Vec<ExternalError>>
    where
        M: IntoIterator<Item = CursorGrabMode>,
    {
        modes
            .into_iter()
            .map(|mode| self.0.set_cursor_grab(mode).err())
            .collect::<Option<Vec<_>>>()
            .map_or(Ok(()), Err)
    }
}

impl EventHandler for Window {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        match event {
            Event::WindowEvent(event) => match event {
                WindowEvent::MouseInput {
                    button: MouseButton::Left,
                    state: ElementState::Pressed,
                    ..
                } => {
                    self.set_cursor_grab([CursorGrabMode::Confined, CursorGrabMode::Locked])
                        .expect("cursor should be grabbable");
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
                    self.set_cursor_grab([CursorGrabMode::None])
                        .unwrap_or_else(|_| unreachable!());
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
    type Target = RawWindow;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Window> for wgpu::SurfaceTarget<'static> {
    fn from(window: Window) -> Self {
        window.0.into()
    }
}
