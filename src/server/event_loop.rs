use super::{Connection, SERVER_CONFIG, ticker::Ticker};
use crate::client::PlayerEvent;
use crossbeam_channel::{Receiver, RecvTimeoutError};
use serde::Deserialize;

pub struct EventLoop {
    connection: Connection,
    connection_rx: Receiver<Connection>,
    player_rx: Receiver<PlayerEvent>,
}

impl EventLoop {
    pub fn new(connection_rx: Receiver<Connection>, player_rx: Receiver<PlayerEvent>) -> Self {
        Self {
            connection: Connection::closed(),
            connection_rx,
            player_rx,
        }
    }

    pub fn run<H>(&mut self, mut handler: H)
    where
        H: for<'a> EventHandler<Event, Context<'a> = &'a Connection>,
    {
        let mut ticker = Ticker::start(SERVER_CONFIG.event_loop.ticks_per_second);
        loop {
            let event = match ticker.recv_timeout(&self.player_rx) {
                Ok(event) => Event::Player(event),
                Err(RecvTimeoutError::Timeout) => Event::Tick,
                Err(RecvTimeoutError::Disconnected) => break,
            };

            if let Some(connection) = self.connection_rx.try_iter().last() {
                self.connection = connection;
            }

            handler.handle(&event, &self.connection);
        }
    }
}

pub trait EventHandler<E> {
    type Context<'a>;

    fn handle(&mut self, event: &E, cx: Self::Context<'_>);
}

pub enum Event {
    Player(PlayerEvent),
    Tick,
}

#[derive(Deserialize)]
pub struct EventLoopConfig {
    pub ticks_per_second: u32,
}
