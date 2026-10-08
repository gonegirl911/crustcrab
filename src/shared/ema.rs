pub struct Ema {
    sample_weight: f32,
    estimate: Option<f32>,
}

impl Ema {
    pub fn new(sample_weight: f32) -> Self {
        Self {
            sample_weight,
            estimate: None,
        }
    }

    pub fn smooth(&mut self, sample: f32) -> f32 {
        let estimate = self.estimate.map_or(sample, |estimate| {
            estimate + (sample - estimate) * self.sample_weight
        });
        self.estimate = Some(estimate);
        estimate
    }

    pub fn get(&self) -> Option<f32> {
        self.estimate
    }
}
