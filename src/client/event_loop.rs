use super::{stopwatch::Stopwatch, ClientEvent};
use crate::server::ServerEvent;
use flume::{Receiver, Sender};
use std::time::Duration;
use winit::{
    event::{Event as RawEvent, StartCause},
    event_loop::{
        ControlFlow, EventLoop as RawEventLoop, EventLoopBuilder as RawEventLoopBuilder,
        EventLoopProxy as RawEventLoopProxy,
    },
};

pub struct EventLoop {
    event_loop: RawEventLoop<ServerEvent>,
    proxy: RawEventLoopProxy<ServerEvent>,
    client_tx: Sender<ClientEvent>,
    server_rx: Receiver<ServerEvent>,
}

impl EventLoop {
    pub fn new(client_tx: Sender<ClientEvent>, server_rx: Receiver<ServerEvent>) -> Self {
        let event_loop = RawEventLoopBuilder::with_user_event().build();
        let proxy = event_loop.create_proxy();
        Self {
            event_loop,
            proxy,
            client_tx,
            server_rx,
        }
    }

    pub fn run<H>(self, mut handler: H) -> !
    where
        H: for<'a> EventHandler<Context<'a> = (&'a mut ControlFlow, Sender<ClientEvent>, Duration)>
            + 'static,
    {
        let mut stopwatch = Stopwatch::start();
        let mut dt = Duration::ZERO;

        self.event_loop.run(move |event, _, control_flow| {
            match event {
                Event::NewEvents(StartCause::Init) | Event::RedrawEventsCleared => {
                    for event in self.server_rx.try_iter() {
                        self.proxy
                            .send_event(event)
                            .unwrap_or_else(|_| unreachable!());
                    }
                }
                Event::MainEventsCleared => dt = stopwatch.lap(),
                _ => {}
            }
            handler.handle(&event, (control_flow, self.client_tx.clone(), dt));
        })
    }
}

impl AsRef<RawEventLoop<ServerEvent>> for EventLoop {
    fn as_ref(&self) -> &RawEventLoop<ServerEvent> {
        &self.event_loop
    }
}

pub trait EventHandler {
    type Context<'a>;

    fn handle(&mut self, event: &Event, cx: Self::Context<'_>);
}

pub type Event<'a> = RawEvent<'a, ServerEvent>;
