use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Clone, Copy, Serialize, Deserialize)]
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
