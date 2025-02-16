use crate::{
    client::ClientEvent,
    server::{
        SERVER_CONFIG, ServerEvent, ServerSender,
        event_loop::{Event, EventHandler},
    },
    shared::utils::{self, Lerp},
};
use nalgebra::{UnitQuaternion, Vector3};
use serde::{Deserialize, Serialize};
use std::{f32::consts::TAU, ops::Range};

#[derive(Clone, Copy)]
pub struct Clock {
    ticks: u16,
    is_client_connected: bool,
}

impl Clock {
    fn send(self, server_tx: &ServerSender) {
        if self.is_client_connected {
            _ = server_tx.send(ServerEvent::TimeUpdated(self.time()));
        }
    }

    fn time(self) -> Time {
        Time { ticks: self.ticks }
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            ticks: SERVER_CONFIG.clock.starting_ticks(),
            is_client_connected: false,
        }
    }
}

impl EventHandler<Event> for Clock {
    type Context<'a> = &'a ServerSender;

    fn handle(&mut self, event: &Event, server_tx: Self::Context<'_>) {
        match event {
            Event::Client(ClientEvent::InitialRenderRequested { .. }) => {
                self.is_client_connected = true;
                self.send(server_tx);
            }
            Event::Tick => {
                self.ticks = (self.ticks + 1) % SERVER_CONFIG.clock.ticks_per_day;
                self.send(server_tx);
            }
            Event::Client(ClientEvent::Disconnected) => {
                self.is_client_connected = false;
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
        let time = SERVER_CONFIG.clock.time(self.ticks);
        let theta = TAU * time;
        UnitQuaternion::new(Vector3::z() * theta)
    }

    pub fn stage(self) -> Stage {
        SERVER_CONFIG.clock.stage(self.ticks)
    }

    pub fn is_am(self) -> bool {
        SERVER_CONFIG.clock.is_am(self.ticks)
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

impl Stage {
    pub fn lerp<T: Lerp>(self, day: T, night: T) -> T {
        utils::lerp(day, night, self.progress())
    }

    pub fn progress(self) -> f32 {
        match self {
            Self::Dawn { progress } => 1.0 - progress,
            Self::Day => 0.0,
            Self::Dusk { progress } => progress,
            Self::Night => 1.0,
        }
    }
}

impl Default for Stage {
    fn default() -> Self {
        Time::default().stage()
    }
}

#[derive(Clone, Copy, Deserialize)]
pub struct ClockState {
    ticks_per_day: u16,
    twilight_duration: u16,
    starting_stage: StartingStage,
}

impl ClockState {
    fn starting_ticks(self) -> u16 {
        match self.starting_stage {
            StartingStage::Dawn => 0,
            StartingStage::Day => self.day_start(),
            StartingStage::Dusk => self.dusk_start(),
            StartingStage::Night => self.night_start(),
        }
    }

    fn time(self, ticks: u16) -> f32 {
        (ticks as i16 - self.horizon() as i16) as f32 / self.ticks_per_day as f32
    }

    fn stage(self, ticks: u16) -> Stage {
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

    fn is_am(self, ticks: u16) -> bool {
        !self.is_pm(ticks)
    }

    fn is_pm(self, ticks: u16) -> bool {
        self.pm_range().contains(&ticks)
    }

    fn dawn_range(self) -> Range<u16> {
        0..self.day_start()
    }

    fn day_range(self) -> Range<u16> {
        self.day_start()..self.dusk_start()
    }

    fn dusk_range(self) -> Range<u16> {
        self.dusk_start()..self.night_start()
    }

    fn pm_range(self) -> Range<u16> {
        self.noon()..self.midnight()
    }

    fn horizon(self) -> u16 {
        self.twilight_duration / 2
    }

    fn day_start(self) -> u16 {
        self.twilight_duration
    }

    fn noon(self) -> u16 {
        self.horizon() + self.ticks_per_day / 4
    }

    fn dusk_start(self) -> u16 {
        self.ticks_per_day / 2
    }

    fn night_start(self) -> u16 {
        self.dusk_start() + self.twilight_duration
    }

    fn midnight(self) -> u16 {
        self.noon() + self.ticks_per_day / 2
    }

    fn inv_lerp(Range { start, end }: Range<u16>, value: u16) -> f32 {
        (value - start) as f32 / (end - 1 - start) as f32
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StartingStage {
    Dawn,
    Day,
    Dusk,
    Night,
}
