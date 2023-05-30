use nalgebra::Vector3;

pub fn magnitude_squared(vector: Vector3<i32>) -> u32 {
    vector.map(|c| c.pow(2)).sum() as u32
}

pub fn div_floor(a: i64, b: i64) -> i64 {
    let d = a / b;
    let r = a % b;
    if (r > 0 && b < 0) || (r < 0 && b > 0) {
        d - 1
    } else {
        d
    }
}

pub const fn div_ceil(a: usize, b: usize) -> usize {
    let d = a / b;
    let r = a % b;
    if r > 0 && b > 0 {
        d + 1
    } else {
        d
    }
}
