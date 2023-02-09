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
    const DAWN_START: u16 = Self::TICKS_PER_DAY / 150 * 30;
    const DAWN_END: u16 = Self::TICKS_PER_DAY / 150 * 41;
    const DAY_START: u16 = Self::DAWN_END + 1;
    const DAY_END: u16 = Self::DUSK_START - 1;
    const DUSK_START: u16 = Self::TICKS_PER_DAY / 150 * 117;
    const DUSK_END: u16 = Self::TICKS_PER_DAY / 150 * 135;

    fn data(&self) -> TimeData {
        TimeData {
            time: self.ticks as f32 / Self::TICKS_PER_DAY as f32,
            stage: self.stage(),
        }
    }

    fn stage(&self) -> Stage {
        match self.ticks {
            Self::DAWN_START..=Self::DAWN_END => Stage::Dawn {
                progress: Self::inv_lerp(
                    Self::DAWN_START as f32,
                    Self::DAWN_END as f32,
                    self.ticks as f32,
                ),
            },
            Self::DAY_START..=Self::DAY_END => Stage::Day,
            Self::DUSK_START..=Self::DUSK_END => Stage::Dusk {
                progress: Self::inv_lerp(
                    Self::DUSK_START as f32,
                    Self::DUSK_END as f32,
                    self.ticks as f32,
                ),
            },
            _ => Stage::Night,
        }
    }

    fn inv_lerp(start: f32, end: f32, value: f32) -> f32 {
        (value - start) / (end - start)
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            ticks: Self::DAWN_START,
        }
    }
}

impl EventHandler<Event> for Clock {
    type Context<'a> = Sender<ServerEvent>;

    fn handle(&mut self, event: &Event, server_tx: Self::Context<'_>) {
        match event {
            Event::ClientEvent(ClientEvent::InitialRenderRequested { .. }) => {
                server_tx
                    .send(ServerEvent::TimeUpdated(self.data()))
                    .unwrap_or_else(|_| unreachable!());
            }
            Event::Tick => {
                self.ticks = (self.ticks + 1) % Self::TICKS_PER_DAY;
                server_tx
                    .send(ServerEvent::TimeUpdated(self.data()))
                    .unwrap_or_else(|_| unreachable!());
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct TimeData {
    pub time: f32,
    pub stage: Stage,
}

#[derive(Clone, Copy, Default)]
pub enum Stage {
    #[default]
    Night,
    Dawn {
        progress: f32,
    },
    Day,
    Dusk {
        progress: f32,
    },
}
