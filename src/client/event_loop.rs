use super::ClientEvent;
use crate::server::ServerEvent;
use flume::Sender;
use std::ops::Deref;
use winit::{
    event::{Event as RawEvent, WindowEvent},
    event_loop::{
        ControlFlow, EventLoop as RawEventLoop, EventLoopBuilder as RawEventLoopBuilder,
        EventLoopProxy as RawEventLoopProxy,
    },
};

pub struct EventLoop {
    event_loop: RawEventLoop<ServerEvent>,
    client_tx: Sender<ClientEvent>,
}

impl EventLoop {
    pub fn new(client_tx: Sender<ClientEvent>) -> Self {
        Self {
            event_loop: Self::event_loop(),
            client_tx,
        }
    }

    pub fn run<H>(self, mut handler: H)
    where
        H: for<'a> EventHandler<Context<'a> = &'a Sender<ClientEvent>> + 'static,
    {
        self.event_loop
            .run(move |event, elwt| {
                handler.handle(&event, &self.client_tx);

                if let Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } = event
                {
                    elwt.exit();
                }
            })
            .expect("event loop should be runnable");
    }

    fn event_loop() -> RawEventLoop<ServerEvent> {
        let event_loop = RawEventLoopBuilder::with_user_event()
            .build()
            .expect("event loop should be buildable");

        event_loop.set_control_flow(ControlFlow::Poll);

        event_loop
    }
}

impl Deref for EventLoop {
    type Target = RawEventLoop<ServerEvent>;

    fn deref(&self) -> &Self::Target {
        &self.event_loop
    }
}

pub trait EventHandler {
    type Context<'a>;

    fn handle(&mut self, event: &Event, cx: Self::Context<'_>);
}

pub type Event = RawEvent<ServerEvent>;

pub type EventLoopProxy = RawEventLoopProxy<ServerEvent>;
