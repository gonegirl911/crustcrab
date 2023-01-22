use super::{ticker::Ticker, ServerEvent};
use crate::client::ClientEvent;
use flume::{Receiver, Sender};

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
        let mut ticker = Ticker::start();

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

    fn handle(&mut self, event: &E, ctx: Self::Context<'_>);
}

pub enum Event {
    Init,
    ClientEvent(ClientEvent),
    Tick,
}
