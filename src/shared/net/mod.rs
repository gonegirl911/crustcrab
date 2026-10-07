pub(crate) mod attach;
pub(crate) mod codec;
pub(crate) mod compression;

use std::time::Duration;

pub const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(10);
pub const KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(30);
