use serde::de::DeserializeOwned;
use std::{fs, path::Path};

pub fn deserialize<P: AsRef<Path>, T: DeserializeOwned>(path: P) -> T {
    let path = path.as_ref();
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to open {}: {e}", path.display()));
    toml::from_str(&contents)
        .unwrap_or_else(|e| panic!("failed to deserialize {}: {e}", path.display()))
}
