use nalgebra::SVector;

pub fn magnitude_squared<const N: usize>(vector: SVector<i32, N>) -> u128 {
    vector.map(|c| (c.unsigned_abs() as u128).pow(2)).sum()
}
