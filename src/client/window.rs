use super::event_loop::{Event, EventHandler};
use std::ops::Deref;
use winit::{
    event::{ElementState, KeyboardInput, MouseButton, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop as RawEventLoop},
    window::{CursorGrabMode, Window as RawWindow, WindowBuilder as RawWindowBuilder},
};

pub struct Window(RawWindow);

impl Window {
    pub fn new<T>(event_loop: &RawEventLoop<T>) -> Self {
        Self(
            RawWindowBuilder::new()
                .with_title("Crustcrab")
                .build(event_loop)
                .expect("window should be creatable"),
        )
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
                    self.0
                        .set_cursor_grab(CursorGrabMode::Locked)
                        .or_else(|_| self.0.set_cursor_grab(CursorGrabMode::Confined))
                        .expect("cursor should be lockable");
                    self.0.set_cursor_visible(false);
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
                    self.0
                        .set_cursor_grab(CursorGrabMode::None)
                        .expect("cursor should be unlockable");
                    self.0.set_cursor_visible(true);
                }
                WindowEvent::CloseRequested => control_flow.set_exit(),
                _ => {}
            },
            Event::MainEventsCleared => self.0.request_redraw(),
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
