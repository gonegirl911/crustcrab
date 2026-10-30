use crate::{
    CONFIG,
    event::{self, Event},
    stopwatch::Stopwatch,
};
use base::{
    math::{
        interpolation::ramp,
        smoothing::{ExponentialSmoother, Weight},
    },
    position::coord::Coord,
};
use domain::{game::clock::DayCycle, protocol::control};
use nalgebra::{UnitQuaternion, Vector3, wrap};
use serde::Deserialize;
use std::{f32::consts::TAU, ops::Range, time::Duration};
use winit::event::WindowEvent;

#[derive(Default)]
pub(super) struct Clock {
    ticks_per_second: u32,
    cycle: DayCycle,
    position: Coord,
    sync_error: ExponentialSmoother,
}

impl Clock {
    pub(super) fn render_time(&self) -> RenderTime {
        RenderTime::new(self.cycle, self.position)
    }

    fn synchronize(&mut self, ticks: u16) {
        let position = self.position.wrapped_into(self.cycle.ticks_per_day as i64);
        let difference = position - ticks as f32;
        let period = self.cycle.ticks_per_day as f32;
        let error = wrap(difference, -period * 0.5, period * 0.5);
        self.sync_error.value = Some(error);
    }

    fn advance(&mut self, dt: Duration) {
        let dt_s = dt.as_secs_f32();
        let correction = self.decay_sync_error(dt_s);
        self.position += self.ticks_per_second as f32 * dt_s + correction;
    }

    fn decay_sync_error(&mut self, dt_s: f32) -> f32 {
        let prev = self.sync_error.value.unwrap_or(0.0);
        let decay_time_s = CONFIG.clock.sync_error_decay_time_s;
        let decay_amount = dt_s / decay_time_s.max(f32::MIN_POSITIVE);
        let weight = Weight::from_exponential_decay(decay_amount);
        let cur = self.sync_error.smooth(0.0, weight);
        cur - prev
    }
}

impl event::Handler for Clock {
    type Context<'a> = &'a Stopwatch;

    fn handle(&mut self, event: &Event, &Stopwatch { dt, .. }: Self::Context<'_>) {
        match *event {
            Event::Control(control::Event::TimeInitialized {
                ticks_per_second,
                cycle,
                ticks,
            }) => {
                *self = Self {
                    ticks_per_second,
                    cycle,
                    position: (ticks as i64).into(),
                    sync_error: Default::default(),
                };
            }
            Event::Control(control::Event::TimeUpdated { ticks }) => {
                self.synchronize(ticks);
            }
            Event::WindowEvent(WindowEvent::RedrawRequested) => {
                self.advance(dt);
            }
            _ => {}
        }
    }
}

pub(super) struct RenderTime {
    cycle: DayCycle,
    pub(super) position: Coord,
    ticks: f32,
    pub(super) nightness: f32,
}

impl RenderTime {
    fn new(cycle: DayCycle, position: Coord) -> Self {
        let ticks = position.wrapped_into(cycle.ticks_per_day as i64);
        let nightness = Self::nightness(cycle, ticks);
        Self {
            cycle,
            position,
            ticks,
            nightness,
        }
    }

    pub(super) fn sun_dir(&self) -> Vector3<f32> {
        self.sky_rotation() * Vector3::x()
    }

    pub(super) fn sky_rotation(&self) -> UnitQuaternion<f32> {
        let anchor = self.cycle.sunrise() as f32;
        let period = self.cycle.ticks_per_day as f32;
        let progress = (self.ticks - anchor) / period;
        let angle = TAU * progress;
        UnitQuaternion::from_axis_angle(&Vector3::z_axis(), angle)
    }

    fn nightness(cycle: DayCycle, ticks: f32) -> f32 {
        let dawn_range = cycle.dawn_range();
        let day_range = cycle.day_range();
        let dusk_range = cycle.dusk_range();
        if ticks < day_range.start as f32 {
            1.0 - Self::progress(dawn_range, ticks)
        } else if ticks < dusk_range.start as f32 {
            0.0
        } else if ticks < cycle.night_start() as f32 {
            Self::progress(dusk_range, ticks)
        } else {
            1.0
        }
    }

    fn progress(Range { start, end }: Range<u16>, ticks: f32) -> f32 {
        ramp(start as f32, end as f32, ticks)
    }
}

#[derive(Deserialize)]
pub(crate) struct Config {
    sync_error_decay_time_s: f32,
}
