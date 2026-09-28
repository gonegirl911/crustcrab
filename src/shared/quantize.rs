pub fn to_unorm16(n: f32) -> u16 {
    (n.clamp(0.0, 1.0) * 65535.0).round() as u16
}

pub fn from_unorm16(n: u16) -> f32 {
    n as f32 / 65535.0
}
