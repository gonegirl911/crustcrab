use crate::{
    client::ClientEvent,
    server::{
        SERVER_CONFIG, ServerEvent, ServerSender,
        event_loop::{Event, EventHandler},
    },
    shared::utils,
};
use nalgebra::{UnitQuaternion, Vector3};
use serde::{Deserialize, Serialize};
use std::{f32::consts::TAU, ops::Range};

pub struct Clock {
    ticks: u16,
}

impl Clock {
    fn send_time(&self, server_tx: &ServerSender) {
        _ = server_tx.send(ServerEvent::TimeUpdated(self.time()));
    }

    fn time(&self) -> Time {
        Time { ticks: self.ticks }
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            ticks: SERVER_CONFIG.clock.starting_ticks(),
        }
    }
}

impl EventHandler<Event> for Clock {
    type Context<'a> = &'a ServerSender;

    fn handle(&mut self, event: &Event, server_tx: Self::Context<'_>) {
        match event {
            Event::Client(ClientEvent::PlayerConnected { .. }) => {
                self.send_time(server_tx);
            }
            Event::Tick => {
                self.ticks = (self.ticks + 1) % SERVER_CONFIG.clock.ticks_per_day;
                self.send_time(server_tx);
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Time {
    ticks: u16,
}

impl Time {
    pub fn sky_rotation(self) -> UnitQuaternion<f32> {
        let config = &SERVER_CONFIG.clock;
        let anchor = config.sunrise() as f32 / config.ticks_per_day as f32;
        let progress = self.progress(0..config.ticks_per_day) - anchor;
        let angle = TAU * progress;
        UnitQuaternion::new(Vector3::z() * angle)
    }

    pub fn nightness(self) -> f32 {
        let config = &SERVER_CONFIG.clock;
        let dawn_range = config.dawn_range();
        let day_range = config.day_range();
        let dusk_range = config.dusk_range();
        if dawn_range.contains(&self.ticks) {
            1.0 - self.progress(dawn_range)
        } else if day_range.contains(&self.ticks) {
            0.0
        } else if dusk_range.contains(&self.ticks) {
            self.progress(dusk_range)
        } else {
            1.0
        }
    }

    fn progress(self, Range { start, end }: Range<u16>) -> f32 {
        utils::inv_lerp(start as f32, (end - 1) as f32, self.ticks as f32)
    }
}

impl Default for Time {
    fn default() -> Self {
        Self {
            ticks: SERVER_CONFIG.clock.starting_ticks(),
        }
    }
}

#[derive(Deserialize)]
pub struct ClockConfig {
    ticks_per_day: u16,
    twilight_duration: u16,
    starting_phase: TimePhase,
}

impl ClockConfig {
    fn starting_ticks(&self) -> u16 {
        match self.starting_phase {
            TimePhase::Dawn => 0,
            TimePhase::Day => self.day_start(),
            TimePhase::Dusk => self.dusk_start(),
            TimePhase::Night => self.night_start(),
        }
    }

    fn dawn_range(&self) -> Range<u16> {
        0..self.day_start()
    }

    fn day_range(&self) -> Range<u16> {
        self.day_start()..self.dusk_start()
    }

    fn dusk_range(&self) -> Range<u16> {
        self.dusk_start()..self.night_start()
    }

    fn sunrise(&self) -> u16 {
        self.twilight_duration / 2
    }

    fn day_start(&self) -> u16 {
        self.twilight_duration
    }

    fn dusk_start(&self) -> u16 {
        self.ticks_per_day / 2
    }

    fn night_start(&self) -> u16 {
        self.dusk_start() + self.twilight_duration
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TimePhase {
    Dawn,
    Day,
    Dusk,
    Night,
}
