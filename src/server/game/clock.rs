use crate::{
    client::PlayerEvent,
    server::{
        ControlEvent, SERVER_CONFIG,
        event_loop::{Event, EventHandler},
    },
};
use crossbeam_channel::Sender;
use serde::Deserialize;
use std::ops::Range;

pub struct Clock {
    ticks: u16,
}

impl Clock {
    fn send_time(&self, control_tx: &Sender<ControlEvent>) {
        _ = control_tx.send(ControlEvent::TimeUpdated { ticks: self.ticks });
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
    type Context<'a> = &'a Sender<ControlEvent>;

    fn handle(&mut self, event: &Event, control_tx: Self::Context<'_>) {
        match event {
            Event::Player(PlayerEvent::JoinRequested { .. }) => {
                self.send_time(control_tx);
            }
            Event::Tick => {
                self.ticks = (self.ticks + 1) % SERVER_CONFIG.clock.ticks_per_day;
                self.send_time(control_tx);
            }
            _ => {}
        }
    }
}

#[derive(Deserialize)]
pub struct ClockConfig {
    pub ticks_per_day: u16,
    twilight_duration: u16,
    starting_phase: TimePhase,
}

impl ClockConfig {
    pub fn starting_ticks(&self) -> u16 {
        match self.starting_phase {
            TimePhase::Dawn => 0,
            TimePhase::Day => self.day_start(),
            TimePhase::Dusk => self.dusk_start(),
            TimePhase::Night => self.night_start(),
        }
    }

    pub fn dawn_range(&self) -> Range<u16> {
        0..self.day_start()
    }

    pub fn day_range(&self) -> Range<u16> {
        self.day_start()..self.dusk_start()
    }

    pub fn dusk_range(&self) -> Range<u16> {
        self.dusk_start()..self.night_start()
    }

    pub fn sunrise(&self) -> u16 {
        self.twilight_duration / 2
    }

    fn day_start(&self) -> u16 {
        self.twilight_duration
    }

    fn dusk_start(&self) -> u16 {
        self.ticks_per_day / 2
    }

    pub fn night_start(&self) -> u16 {
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
