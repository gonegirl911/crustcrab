use super::event_loop::{Event, EventHandler, EventLoop};
use winit::{
    event::{ElementState, KeyboardInput, MouseButton, VirtualKeyCode, WindowEvent},
    event_loop::ControlFlow,
    window::{CursorGrabMode, Window as RawWindow, WindowBuilder as RawWindowBuilder},
};

pub struct Window {
    window: RawWindow,
}

impl Window {
    pub fn new(event_loop: &EventLoop) -> Self {
        Self {
            window: RawWindowBuilder::new()
                .with_title("Crustcrab")
                .build(event_loop.as_ref())
                .expect("window should be creatable"),
        }
    }
}

impl AsRef<RawWindow> for Window {
    fn as_ref(&self) -> &RawWindow {
        &self.window
    }
}

impl EventHandler for Window {
    type Context<'a> = &'a mut ControlFlow;

    fn handle(&mut self, event: &Event, control_flow: Self::Context<'_>) {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::MouseInput {
                    button: MouseButton::Left,
                    state: ElementState::Pressed,
                    ..
                } => {
                    self.window
                        .set_cursor_grab(CursorGrabMode::Locked)
                        .or_else(|_| self.window.set_cursor_grab(CursorGrabMode::Confined))
                        .expect("cursor should be lockable");
                    self.window.set_cursor_visible(false);
                }
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    self.window
                        .set_cursor_grab(CursorGrabMode::None)
                        .expect("cursor should be unlockable");
                    self.window.set_cursor_visible(true);
                }
                WindowEvent::CloseRequested => control_flow.set_exit(),
                _ => {}
            },
            Event::MainEventsCleared => self.window.request_redraw(),
            _ => {}
        }
    }
}
