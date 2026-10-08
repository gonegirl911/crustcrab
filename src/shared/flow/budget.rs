pub struct Budget {
    rate: f32,
    carry: f32,
}

impl Budget {
    pub fn new(rate: f32) -> Self {
        Self { rate, carry: 0.0 }
    }

    pub fn draw(&mut self) -> usize {
        self.carry += self.rate;
        let drawn = self.carry as usize;
        self.carry -= drawn as f32;
        drawn
    }

    pub fn set_rate(&mut self, rate: f32) {
        self.rate = rate;
    }
}
