use crate::{
    client::ClientEvent,
    server::{
        event_loop::{Event, EventHandler},
        ServerEvent,
    },
};
use flume::Sender;
use std::ops::Range;

pub struct Clock {
    ticks: u16,
}

impl Clock {
    const TICKS_PER_DAY: u16 = 24000;
    const DAWN_START: u16 = Self::TICKS_PER_DAY / 150 * 31;
    const DAY_START: u16 = Self::TICKS_PER_DAY / 150 * 41;
    const DUSK_START: u16 = Self::TICKS_PER_DAY / 150 * 117;
    const NIGHT_START: u16 = Self::TICKS_PER_DAY / 150 * 135;
    const DAWN_RANGE: Range<u16> = Self::DAWN_START..Self::DAY_START;
    const DAY_RANGE: Range<u16> = Self::DAY_START..Self::DUSK_START;
    const DUSK_RANGE: Range<u16> = Self::DUSK_START..Self::NIGHT_START;

    fn send_data(&self, server_tx: Sender<ServerEvent>) {
        server_tx
            .send(ServerEvent::TimeUpdated(self.data()))
            .unwrap_or_else(|_| unreachable!());
    }

    fn data(&self) -> TimeData {
        TimeData {
            time: self.ticks as f32 / Self::TICKS_PER_DAY as f32,
            stage: self.stage(),
        }
    }

    fn stage(&self) -> Stage {
        if Self::DAWN_RANGE.contains(&self.ticks) {
            Stage::Dawn {
                progress: Self::inv_lerp(Self::DAWN_RANGE, self.ticks),
            }
        } else if Self::DAY_RANGE.contains(&self.ticks) {
            Stage::Day
        } else if Self::DUSK_RANGE.contains(&self.ticks) {
            Stage::Dusk {
                progress: Self::inv_lerp(Self::DUSK_RANGE, self.ticks),
            }
        } else {
            Stage::Night
        }
    }

    fn inv_lerp(Range { start, end }: Range<u16>, value: u16) -> f32 {
        (value - start) as f32 / (end - 1 - start) as f32
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            ticks: Self::NIGHT_START,
        }
    }
}

impl EventHandler<Event> for Clock {
    type Context<'a> = Sender<ServerEvent>;

    fn handle(&mut self, event: &Event, server_tx: Self::Context<'_>) {
        match event {
            Event::ClientEvent(ClientEvent::InitialRenderRequested { .. }) => {
                self.send_data(server_tx);
            }
            Event::Tick => {
                self.ticks = (self.ticks + 1) % Self::TICKS_PER_DAY;
                self.send_data(server_tx);
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
