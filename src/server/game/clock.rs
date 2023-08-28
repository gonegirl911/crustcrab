use crate::{
    client::ClientEvent,
    server::{
        event_loop::{Event, EventHandler},
        ServerEvent,
    },
};
use flume::Sender;
use nalgebra::{UnitQuaternion, Vector3};
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::{f32::consts::TAU, fs, ops::Range};

pub struct Clock {
    ticks: u16,
}

impl Clock {
    fn send(&self, server_tx: Sender<ServerEvent>) {
        server_tx
            .send(ServerEvent::TimeUpdated(self.time()))
            .unwrap_or_else(|_| unreachable!());
    }

    fn time(&self) -> Time {
        Time { ticks: self.ticks }
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            ticks: CLOCK_STATE.starting_ticks(),
        }
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
                self.ticks = (self.ticks + 1) % CLOCK_STATE.ticks_per_day;
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
    pub fn earth_rotation(self) -> UnitQuaternion<f32> {
        let time = self.ticks as f32 / CLOCK_STATE.ticks_per_day as f32;
        let theta = TAU * time;
        UnitQuaternion::from_scaled_axis(Vector3::z() * theta)
    }

    pub fn stage(self) -> Stage {
        CLOCK_STATE.stage(self.ticks)
    }

    pub fn is_am(self) -> bool {
        CLOCK_STATE.is_am(self.ticks)
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

#[derive(Deserialize)]
struct ClockState {
    ticks_per_day: u16,
    twilight_duration: u16,
    starting_stage: StartingStage,
}

impl ClockState {
    fn starting_ticks(&self) -> u16 {
        match self.starting_stage {
            StartingStage::Dawn => self.dawn_start(),
            StartingStage::Day => self.day_start(),
            StartingStage::Dusk => self.dusk_start(),
            StartingStage::Night => self.night_start(),
        }
    }

    fn stage(&self, ticks: u16) -> Stage {
        if self.dawn_range().contains(&ticks) {
            Stage::Dawn {
                progress: Self::inv_lerp(self.dawn_range(), ticks),
            }
        } else if self.day_range().contains(&ticks) {
            Stage::Day
        } else if self.dusk_range().contains(&ticks) {
            Stage::Dusk {
                progress: Self::inv_lerp(self.dusk_range(), ticks),
            }
        } else {
            Stage::Night
        }
    }

    fn is_am(&self, ticks: u16) -> bool {
        !self.is_pm(ticks)
    }

    fn is_pm(&self, ticks: u16) -> bool {
        self.pm_range().contains(&ticks)
    }

    fn dawn_range(&self) -> Range<u16> {
        self.dawn_start()..self.day_start()
    }

    fn day_range(&self) -> Range<u16> {
        self.day_start()..self.dusk_start()
    }

    fn dusk_range(&self) -> Range<u16> {
        self.dusk_start()..self.night_start()
    }

    fn pm_range(&self) -> Range<u16> {
        self.noon()..self.midnight()
    }

    fn dawn_start(&self) -> u16 {
        0
    }

    fn day_start(&self) -> u16 {
        self.twilight_duration
    }

    fn noon(&self) -> u16 {
        self.ticks_per_day / 4
    }

    fn dusk_start(&self) -> u16 {
        self.night_start() - self.twilight_duration
    }

    fn night_start(&self) -> u16 {
        self.ticks_per_day / 2
    }

    fn midnight(&self) -> u16 {
        self.ticks_per_day / 4 * 3
    }

    fn inv_lerp(Range { start, end }: Range<u16>, value: u16) -> f32 {
        (value - start) as f32 / (end - 1 - start) as f32
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum StartingStage {
    Dawn,
    Day,
    Dusk,
    Night,
}

static CLOCK_STATE: Lazy<ClockState> = Lazy::new(|| {
    toml::from_str(&fs::read_to_string("assets/clock.toml").expect("file should exist"))
        .expect("file should be valid")
});
