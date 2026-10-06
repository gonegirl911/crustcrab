pub mod attach;
pub mod codec;
pub mod compression;

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Unexpected},
};

#[derive(Clone, Copy, Serialize)]
pub struct ConnectionSettings {
    pub keepalive_interval_ms: u64,
}

impl<'de> Deserialize<'de> for ConnectionSettings {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let keepalive_interval_ms = Deserialize::deserialize(deserializer)?;

        if keepalive_interval_ms < MIN_KEEPALIVE_INTERVAL_MS {
            return Err(de::Error::invalid_value(
                Unexpected::Unsigned(keepalive_interval_ms),
                &&*format!("a value below {MIN_KEEPALIVE_INTERVAL_MS}"),
            ));
        }

        Ok(Self {
            keepalive_interval_ms,
        })
    }
}

pub const MIN_KEEPALIVE_INTERVAL_MS: u64 = 100;
