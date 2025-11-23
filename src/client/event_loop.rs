use crate::server::ServerEvent;
use winit::event::{DeviceEvent, WindowEvent};

pub trait EventHandler {
    type Context<'a>;

    fn handle(&mut self, event: &Event, cx: Self::Context<'_>);
}

pub enum Event {
    Resumed,
    ServerEvent(ServerEvent),
    WindowEvent(WindowEvent),
    DeviceEvent(DeviceEvent),
    AboutToWait,
}
