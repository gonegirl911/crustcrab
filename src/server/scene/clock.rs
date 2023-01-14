use crate::{
    client::ClientEvent,
    server::{
        event_loop::{Event, EventHandler},
        ServerEvent,
    },
};
use flume::Sender;

pub struct Clock {
    ticks: u16,
}

impl Clock {
    const TICKS_PER_DAY: u16 = 24000;

    fn time(&self) -> f32 {
        self.ticks as f32 / Self::TICKS_PER_DAY as f32
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            ticks: Self::TICKS_PER_DAY / 4,
        }
    }
}

impl EventHandler<Event> for Clock {
    type Context<'a> = Sender<ServerEvent>;

    fn handle(&mut self, event: &Event, server_tx: Self::Context<'_>) {
        match event {
            Event::ClientEvent(ClientEvent::InitialRenderRequested { .. }) => {
                server_tx
                    .send(ServerEvent::TimeUpdated { time: self.time() })
                    .unwrap_or_else(|_| unreachable!());
            }
            Event::Tick => {
                self.ticks = (self.ticks + 1) % Self::TICKS_PER_DAY;
                server_tx
                    .send(ServerEvent::TimeUpdated { time: self.time() })
                    .unwrap_or_else(|_| unreachable!());
            }
            _ => {}
        }
    }
}
