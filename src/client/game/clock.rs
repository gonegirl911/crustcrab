use crate::{
    client::{
        CLIENT_CONFIG,
        event_loop::{Event, EventHandler},
    },
    server::{ControlEvent, SERVER_CONFIG},
    shared::utils,
};
use nalgebra::{UnitQuaternion, Vector3};
use serde::Deserialize;
use std::{f64::consts::TAU, ops::Range, time::Duration};
use winit::event::WindowEvent;

pub struct Clock {
    anchor: f64,
    anchor_age: f64,
    prev_ticks: u16,
    error: f64,
}

impl Clock {
    pub fn time(&self) -> RenderTime {
        RenderTime {
            ticks: self.extrapolated_ticks() + self.error,
        }
    }

    fn extrapolated_ticks(&self) -> f64 {
        let ticks_per_second = SERVER_CONFIG.event_loop.ticks_per_second as f64;
        self.anchor + self.anchor_age * ticks_per_second
    }

    fn reanchor(&mut self, ticks: u16) {
        let extrapolated_ticks = self.extrapolated_ticks();
        let ticks_per_day = SERVER_CONFIG.clock.ticks_per_day as f64;
        self.anchor += (ticks as f64 - self.prev_ticks as f64).rem_euclid(ticks_per_day);
        self.anchor_age = 0.0;
        self.prev_ticks = ticks;
        self.error += extrapolated_ticks - self.anchor;
    }

    fn advance(&mut self, dt: Duration) {
        self.anchor_age += dt.as_secs_f64();
        self.decay_error(dt);
    }

    fn decay_error(&mut self, dt: Duration) {
        let decay_time_s = CLIENT_CONFIG.clock.error_decay_time_ms as f64 / 1000.0;
        self.error *= (-dt.as_secs_f64() / decay_time_s).exp();
    }
}

impl Default for Clock {
    fn default() -> Self {
        let starting_ticks = SERVER_CONFIG.clock.starting_ticks();
        Self {
            anchor: starting_ticks as f64,
            anchor_age: 0.0,
            prev_ticks: starting_ticks,
            error: 0.0,
        }
    }
}

impl EventHandler for Clock {
    type Context<'a> = Duration;

    fn handle(&mut self, event: &Event, dt: Self::Context<'_>) {
        match *event {
            Event::ControlEvent(ControlEvent::TimeUpdated { ticks }) => {
                self.reanchor(ticks);
            }
            Event::WindowEvent(WindowEvent::RedrawRequested) => {
                self.advance(dt);
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy)]
pub struct RenderTime {
    pub ticks: f64,
}

impl RenderTime {
    pub fn sun_dir(&self) -> Vector3<f32> {
        self.sky_rotation() * Vector3::x()
    }

    pub fn sky_rotation(&self) -> UnitQuaternion<f32> {
        let config = &SERVER_CONFIG.clock;
        let anchor = config.sunrise() as f64 / config.ticks_per_day as f64;
        let progress = self.ticks / (config.ticks_per_day - 1) as f64 - anchor;
        let angle = TAU * progress.rem_euclid(1.0);
        UnitQuaternion::new(Vector3::z() * angle as f32)
    }

    pub fn nightness(&self) -> f32 {
        let config = &SERVER_CONFIG.clock;
        let ticks = self.ticks.rem_euclid(config.ticks_per_day as f64) as f32;
        let dawn_range = config.dawn_range();
        let day_range = config.day_range();
        let dusk_range = config.dusk_range();
        if ticks < day_range.start as f32 {
            1.0 - Self::progress(ticks, dawn_range)
        } else if ticks < dusk_range.start as f32 {
            0.0
        } else if ticks < config.night_start() as f32 {
            Self::progress(ticks, dusk_range)
        } else {
            1.0
        }
    }

    fn progress(ticks: f32, Range { start, end }: Range<u16>) -> f32 {
        utils::inv_lerp(start as f32, (end - 1) as f32, ticks)
    }
}

#[derive(Deserialize)]
pub struct ClockConfig {
    error_decay_time_ms: u64,
}
