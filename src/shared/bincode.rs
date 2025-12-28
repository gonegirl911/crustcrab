use bincode::{
    config::{self, Config},
    error::{DecodeError, EncodeError},
};
use serde::{Serialize, de::DeserializeOwned};
use std::io::{Read, Write};

pub fn serialize_into<T: Serialize, W: Write>(t: T, mut dst: W) -> Result<usize, SerializeError> {
    bincode::serde::encode_into_std_write(t, &mut dst, config())
}

pub fn deserialize_from<T: DeserializeOwned, R: Read>(mut src: R) -> Result<T, DeserializeError> {
    bincode::serde::decode_from_std_read(&mut src, config())
}

const fn config() -> impl Config {
    config::standard()
}

pub type SerializeError = EncodeError;

pub type DeserializeError = DecodeError;
