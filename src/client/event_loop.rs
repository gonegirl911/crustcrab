use crate::server::ServerEvent;
use winit::{
    event::Event as RawEvent,
    event_loop::{EventLoop as RawEventLoop, EventLoopProxy as RawEventLoopProxy},
};

pub type EventLoop = RawEventLoop<ServerEvent>;

pub trait EventHandler {
    type Context<'a>;

    fn handle(&mut self, event: &Event, cx: Self::Context<'_>);
}

pub type Event = RawEvent<ServerEvent>;

pub type EventLoopProxy = RawEventLoopProxy<ServerEvent>;
