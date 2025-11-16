use crate::server::ServerEvent;
use crossbeam_channel::{SendError, Sender};
use winit::{
    event::{DeviceEvent, WindowEvent},
    event_loop::{EventLoop as RawEventLoop, EventLoopProxy as RawEventLoopProxy},
};

pub type EventLoop = RawEventLoop;

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

#[derive(Clone)]
pub struct EventLoopProxy {
    proxy: RawEventLoopProxy,
    server_tx: Sender<ServerEvent>,
}

impl EventLoopProxy {
    pub fn new(proxy: RawEventLoopProxy, server_tx: Sender<ServerEvent>) -> Self {
        Self { proxy, server_tx }
    }

    pub fn send_event(&self, event: ServerEvent) -> Result<(), SendError<ServerEvent>> {
        self.server_tx.send(event)?;
        self.proxy.wake_up();
        Ok(())
    }
}
