use super::{SERVER_CONFIG, ticker::Ticker};
use crate::client::{ClientEvent, event_loop::EventLoopProxy};
use crossbeam_channel::{Receiver, RecvTimeoutError};
use serde::Deserialize;

pub struct EventLoop {
    proxy: EventLoopProxy,
    client_rx: Receiver<ClientEvent>,
}

impl EventLoop {
    pub fn new(proxy: EventLoopProxy, client_rx: Receiver<ClientEvent>) -> Self {
        Self { proxy, client_rx }
    }

    pub fn run<H>(self, mut handler: H)
    where
        H: for<'a> EventHandler<Event, Context<'a> = &'a EventLoopProxy>,
    {
        let mut ticker = Ticker::start(SERVER_CONFIG.event_loop.ticks_per_second);
        handler.handle(&Event::Init, &self.proxy);
        loop {
            handler.handle(
                &match ticker.recv_timeout(&self.client_rx) {
                    Ok(event) => Event::Client(event),
                    Err(RecvTimeoutError::Timeout) => Event::Tick,
                    Err(RecvTimeoutError::Disconnected) => break,
                },
                &self.proxy,
            );
        }
    }
}

pub trait EventHandler<E> {
    type Context<'a>;

    fn handle(&mut self, event: &E, cx: Self::Context<'_>);
}

pub enum Event {
    Init,
    Client(ClientEvent),
    Tick,
}

#[derive(Deserialize)]
pub struct EventLoopConfig {
    ticks_per_second: u32,
}
