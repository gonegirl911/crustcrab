pub mod attach;
pub mod codec;

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct ConnectionSettings {
    pub keepalive_interval: Duration,
}
