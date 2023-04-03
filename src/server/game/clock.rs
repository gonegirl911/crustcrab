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
    const DAWN_RANGE: Range<u16> = Self::DAWN_START..Self::DAY_START;
    const DAY_RANGE: Range<u16> = Self::DAY_START..Self::DUSK_START;
    const DUSK_RANGE: Range<u16> = Self::DUSK_START..Self::NIGHT_START;

    fn send(&self, server_tx: Sender<ServerEvent>) {
        server_tx
            .send(ServerEvent::TimeUpdated(self.data()))
            .unwrap_or_else(|_| unreachable!());
    }

    fn data(&self) -> TimeData {
        TimeData {
            sun_dir: self.sun_dir(),
            stage: self.stage(),
        }
    }

    fn sun_dir(&self) -> Vector3<f32> {
        let theta = self.time() * TAU;
        vector![theta.cos(), theta.sin(), 0.0]
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

    fn time(&self) -> f32 {
        self.ticks as f32 / Self::TICKS_PER_DAY as f32
    }

    fn inv_lerp(Range { start, end }: Range<u16>, value: u16) -> f32 {
        (value - start) as f32 / (end - 1 - start) as f32
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
pub struct TimeData {
    pub sun_dir: Vector3<f32>,
    pub stage: Stage,
}

impl Default for TimeData {
    fn default() -> Self {
        Clock::default().data()
    }
}

#[derive(Clone, Copy)]
pub enum Stage {
    Dawn { progress: f32 },
    Day,
    Dusk { progress: f32 },
    Night,
}
