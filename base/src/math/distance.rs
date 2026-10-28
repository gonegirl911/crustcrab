use nalgebra::SVector;

pub fn magnitude_squared<const D: usize>(vector: &SVector<i32, D>) -> u128 {
    vector.map(|c| (c.unsigned_abs() as u128).pow(2)).sum()
}
