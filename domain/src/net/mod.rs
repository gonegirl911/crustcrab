pub mod attach;
pub mod codec;

use std::time::Duration;

pub const KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(30);
