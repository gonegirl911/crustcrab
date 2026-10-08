use crate::{
    client::PlayerEvent,
    server::{
        ControlEvent, SERVER_CONFIG,
        connection::ConnectionRegistry,
        event_loop::{Event, EventHandler},
    },
    shared::pacer::Pacer,
};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Unexpected},
};
use std::{
    ops::Range,
    time::{Duration, Instant},
};

pub struct Clock {
    ticks: u16,
    pacer: Pacer,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            ticks: SERVER_CONFIG.clock.starting_ticks(),
            pacer: Pacer::new(TIME_REPORT_GAP),
        }
    }
}

impl EventHandler<Event> for Clock {
    type Context<'a> = &'a ConnectionRegistry;

    fn handle(&mut self, event: &Event, connections: Self::Context<'_>) {
        match *event {
            Event::Player(id, PlayerEvent::JoinRequested { .. }) => {
                let recipient = connections.one(id);
                recipient.send(ControlEvent::TimeInitialized {
                    ticks_per_second: SERVER_CONFIG.event_loop.ticks_per_second,
                    cycle: SERVER_CONFIG.clock.cycle,
                    ticks: self.ticks,
                });
            }
            Event::Tick => {
                self.ticks = (self.ticks + 1) % SERVER_CONFIG.clock.cycle.ticks_per_day;
                self.pacer.fire(Instant::now(), || {
                    connections
                        .all()
                        .send(ControlEvent::TimeUpdated { ticks: self.ticks });
                });
            }
            _ => {}
        }
    }
}

#[derive(Deserialize)]
pub struct ClockConfig {
    #[serde(flatten)]
    cycle: DayCycle,
    starting_phase: TimePhase,
}

impl ClockConfig {
    fn starting_ticks(&self) -> u16 {
        match self.starting_phase {
            TimePhase::Dawn => 0,
            TimePhase::Day => self.cycle.day_start(),
            TimePhase::Dusk => self.cycle.dusk_start(),
            TimePhase::Night => self.cycle.night_start(),
        }
    }
}

#[derive(Clone, Copy, Serialize)]
pub struct DayCycle {
    pub ticks_per_day: u16,
    pub twilight_duration: u16,
}

impl DayCycle {
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

impl Default for DayCycle {
    fn default() -> Self {
        Self {
            ticks_per_day: u16::MAX,
            twilight_duration: u16::MAX / 2,
        }
    }
}

impl<'de> Deserialize<'de> for DayCycle {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct RawDayCycle {
            ticks_per_day: u16,
            twilight_duration: u16,
        }

        let data = RawDayCycle::deserialize(deserializer)?;

        if data.ticks_per_day < 2 {
            return Err(de::Error::invalid_value(
                Unexpected::Unsigned(data.ticks_per_day as u64),
                &"a value above 1",
            ));
        }
        if data.twilight_duration > data.ticks_per_day / 2 {
            return Err(de::Error::invalid_value(
                Unexpected::Unsigned(data.twilight_duration as u64),
                &"a duration not exceeding half a day",
            ));
        }

        Ok(Self {
            ticks_per_day: data.ticks_per_day,
            twilight_duration: data.twilight_duration,
        })
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

const TIME_REPORT_GAP: Duration = Duration::from_secs(1);
