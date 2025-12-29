use serde::de::DeserializeOwned;
use std::{fs, path::Path};
use toml::from_str;

pub fn deserialize<P: AsRef<Path>, T: DeserializeOwned>(path: P) -> T {
    let path = path.as_ref();
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    from_str(&contents).unwrap_or_else(|e| panic!("failed to deserialize {}: {e}", path.display()))
}
