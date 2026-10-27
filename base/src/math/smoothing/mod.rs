mod estimator;

pub use estimator::Estimator;

use super::interpolation::lerp;

#[derive(Default)]
pub struct ExponentialSmoother {
    pub value: Option<f32>,
}

impl ExponentialSmoother {
    pub fn smooth(&mut self, sample: f32, weight: Weight) -> f32 {
        let value = match self.value {
            Some(value) if sample > value => lerp(value, sample, weight.rise),
            Some(value) => lerp(value, sample, weight.fall),
            None => sample,
        };
        self.value = Some(value);
        value
    }
}

pub struct Weight {
    rise: f32,
    fall: f32,
}

impl Weight {
    fn symmetric(value: f32) -> Self {
        Self {
            rise: value,
            fall: value,
        }
    }

    pub fn asymmetric(rise: f32, fall: f32) -> Self {
        Self { rise, fall }
    }

    pub fn from_exponential_decay(amount: f32) -> Self {
        Self::symmetric(-(-amount).exp_m1())
    }
}
