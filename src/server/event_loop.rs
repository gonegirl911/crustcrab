use super::{ticker::Ticker, ServerEvent, ServerSettings};
use crate::client::ClientEvent;
use flume::{Receiver, Sender};

pub struct EventLoop {
    server_tx: Sender<ServerEvent>,
    client_rx: Receiver<ClientEvent>,
    ticks_per_second: u32,
}

impl EventLoop {
    pub fn new(
        server_tx: Sender<ServerEvent>,
        client_rx: Receiver<ClientEvent>,
        settings: &ServerSettings,
    ) -> Self {
        Self {
            server_tx,
            client_rx,
            ticks_per_second: settings.ticks_per_second,
        }
    }

    pub fn run<H>(self, mut handler: H) -> !
    where
        H: for<'a> EventHandler<Event, Context<'a> = Sender<ServerEvent>> + Send,
    {
        let mut ticker = Ticker::start(self.ticks_per_second);

        handler.handle(&Event::Init, self.server_tx.clone());
        loop {
            for event in self.client_rx.try_iter() {
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
