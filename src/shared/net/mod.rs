pub mod attach;
pub mod codec;
pub mod compression;

use serde::{Deserialize, Deserializer, Serialize, de};

#[derive(Clone, Copy, Serialize)]
pub struct ConnectionSettings {
    pub keepalive_interval_ms: u64,
}

impl<'de> Deserialize<'de> for ConnectionSettings {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let keepalive_interval_ms = u64::deserialize(deserializer)?;

        if keepalive_interval_ms < MIN_KEEPALIVE_INTERVAL_MS {
            return Err(de::Error::custom(format!(
                "keepalive interval ({keepalive_interval_ms}ms) \
                below minimum threshold ({MIN_KEEPALIVE_INTERVAL_MS}ms)"
            )));
        }

        Ok(Self {
            keepalive_interval_ms,
        })
    }
}

const MIN_KEEPALIVE_INTERVAL_MS: u64 = 100;
