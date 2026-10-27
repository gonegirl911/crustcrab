use nalgebra::Point3;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Data {
    pub coords: Point3<i32>,
}
