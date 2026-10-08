use super::{
    ControlEvent, SERVER_CONFIG,
    connection::{ConnectionEvent, ConnectionId, ConnectionRegistry},
    gate::Gate,
    ticker::Ticker,
};
use crate::client::PlayerEvent;
use crossbeam_channel::{Receiver, RecvTimeoutError};
use serde::Deserialize;
use std::time::Instant;

pub struct EventLoop {
    connection_rx: Receiver<ConnectionEvent>,
    player_rx: Receiver<(ConnectionId, PlayerEvent)>,
    connections: ConnectionRegistry,
    gate: Gate,
}

impl EventLoop {
    pub fn new(
        connection_rx: Receiver<ConnectionEvent>,
        player_rx: Receiver<(ConnectionId, PlayerEvent)>,
    ) -> Self {
        Self {
            connection_rx,
            player_rx,
            connections: Default::default(),
            gate: Default::default(),
        }
    }

    pub fn run<H>(&mut self, mut handler: H)
    where
        H: for<'a> EventHandler<Event, Context<'a> = &'a ConnectionRegistry>,
    {
        let mut ticker = Ticker::start(SERVER_CONFIG.event_loop.ticks_per_second);
        loop {
            let player_event = match ticker.recv_timeout(&self.player_rx) {
                Ok(event) => Some(event),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => break,
            };

            for connection_event in self.connection_rx.try_iter() {
                self.connections.handle(&connection_event, ());
                self.gate.handle(&connection_event, ());
                handler.handle(&Event::Connection(connection_event), &self.connections);
            }

            let event = match player_event {
                Some((id, event)) => self.gate.admit(id, event, &self.connections),
                None => {
                    let now = Instant::now();
                    self.gate.keep_alive(&self.connections, now);
                    Some(Event::Tick(now))
                }
            };

            if let Some(event) = event {
                handler.handle(&event, &self.connections);
                if let Event::Player(id, PlayerEvent::JoinRequested { .. }) = event {
                    self.connections.one(id).send(ControlEvent::JoinFinished);
                }
            }
        }
    }
}

pub trait EventHandler<E> {
    type Context<'a>;

    fn handle(&mut self, event: &E, cx: Self::Context<'_>);
}

pub enum Event {
    Connection(ConnectionEvent),
    Player(ConnectionId, PlayerEvent),
    Tick(Instant),
}

#[derive(Deserialize)]
pub struct EventLoopConfig {
    pub ticks_per_second: u32,
}
