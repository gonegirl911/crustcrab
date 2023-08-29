use super::{ticker::Ticker, ServerEvent, SERVER_CONFIG};
use crate::client::ClientEvent;
use flume::{Receiver, Sender};
use serde::Deserialize;

pub struct EventLoop {
    server_tx: Sender<ServerEvent>,
    client_rx: Receiver<ClientEvent>,
}

impl EventLoop {
    pub fn new(server_tx: Sender<ServerEvent>, client_rx: Receiver<ClientEvent>) -> Self {
        Self {
            server_tx,
            client_rx,
        }
    }

    pub fn run<H>(self, mut handler: H) -> !
    where
        H: for<'a> EventHandler<Event, Context<'a> = Sender<ServerEvent>> + Send,
    {
        let mut ticker = Ticker::start(SERVER_CONFIG.event_loop.ticks_per_second);

        handler.handle(&Event::Init, self.server_tx.clone());
        loop {
            for event in self.client_rx.drain() {
                handler.handle(&Event::ClientEvent(event), self.server_tx.clone());
            }
            ticker.wait(|| handler.handle(&Event::Tick, self.server_tx.clone()));
        }
    }
}

pub trait EventHandler<E> {
    type Context<'a>;

    fn handle(&mut self, event: &E, cx: Self::Context<'_>);
}

pub enum Event {
    Init,
    ClientEvent(ClientEvent),
    Tick,
}

#[derive(Deserialize)]
pub struct EventLoopConfig {
    ticks_per_second: u32,
}
