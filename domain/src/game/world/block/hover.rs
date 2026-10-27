use base::geometry::bounding::aabb::Aabb3;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Data {
    pub hitbox: Aabb3,
}
