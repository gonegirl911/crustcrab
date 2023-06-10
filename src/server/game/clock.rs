use crate::{
    client::ClientEvent,
    server::{
        event_loop::{Event, EventHandler},
        ServerEvent,
    },
};
use flume::Sender;
use nalgebra::{vector, Vector3};
use std::{f32::consts::TAU, ops::Range};

#[derive(Default)]
pub struct Clock {
    ticks: u16,
}

impl Clock {
    const TICKS_PER_DAY: u16 = 24000;
    const DAWN_START: u16 = 0;
    const DAY_START: u16 = 500;
    const DUSK_START: u16 = 11500;
    const NIGHT_START: u16 = 12000;

    fn send(&self, server_tx: Sender<ServerEvent>) {
        server_tx
            .send(ServerEvent::TimeUpdated(self.time()))
            .unwrap_or_else(|_| unreachable!());
    }

    fn time(&self) -> Time {
        Time { ticks: self.ticks }
    }
}

impl EventHandler<Event> for Clock {
    type Context<'a> = Sender<ServerEvent>;

    fn handle(&mut self, event: &Event, server_tx: Self::Context<'_>) {
        match event {
            Event::ClientEvent(ClientEvent::InitialRenderRequested { .. }) => {
                self.send(server_tx);
            }
            Event::Tick => {
                self.ticks = (self.ticks + 1) % Self::TICKS_PER_DAY;
                self.send(server_tx);
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy)]
pub struct Time {
    ticks: u16,
}

impl Time {
    const DAWN_RANGE: Range<u16> = Clock::DAWN_START..Clock::DAY_START;
    const DAY_RANGE: Range<u16> = Clock::DAY_START..Clock::DUSK_START;
    const DUSK_RANGE: Range<u16> = Clock::DUSK_START..Clock::NIGHT_START;

    pub fn sun_dir(&self) -> Vector3<f32> {
        let time = self.ticks as f32 / Clock::TICKS_PER_DAY as f32;
        let theta = TAU * time;
        vector![theta.cos(), theta.sin(), 0.0]
    }

    pub fn stage(&self) -> Stage {
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

impl Default for Time {
    fn default() -> Self {
        Clock::default().time()
    }
}

#[derive(Clone, Copy)]
pub enum Stage {
    Dawn { progress: f32 },
    Day,
    Dusk { progress: f32 },
    Night,
}
