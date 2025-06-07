use crate::server::ServerEvent;
use winit::{
    event::{DeviceEvent, WindowEvent},
    event_loop::{EventLoop as RawEventLoop, EventLoopProxy as RawEventLoopProxy},
};

pub type EventLoop = RawEventLoop<ServerEvent>;

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

pub type EventLoopProxy = RawEventLoopProxy<ServerEvent>;
