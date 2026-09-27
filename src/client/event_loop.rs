use crate::server::{ChunkEvent, ControlEvent};
use winit::event::{DeviceEvent, WindowEvent};

pub trait EventHandler {
    type Context<'a>;

    fn handle(&mut self, event: &Event, cx: Self::Context<'_>);
}

pub enum Event {
    Resumed,
    ControlEvent(ControlEvent),
    ChunkEvent(ChunkEvent),
    WindowEvent(WindowEvent),
    DeviceEvent(DeviceEvent),
    AboutToWait,
}
