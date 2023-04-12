use nalgebra::SVector;

pub fn magnitude_squared<const D: usize>(vector: SVector<i32, D>) -> u32 {
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

pub fn div_ceil(a: usize, b: usize) -> usize {
    let d = a / b;
    let r = a % b;
    if r > 0 && b > 0 {
        d + 1
    } else {
        d
    }
}
