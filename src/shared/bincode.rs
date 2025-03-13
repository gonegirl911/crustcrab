use serde::{Serialize, de::DeserializeOwned};
use std::io::{Read, Write};

pub fn serialize_into<T: Serialize, W: Write>(t: T, mut dst: W) -> Result<usize, SerializeError> {
    bincode::serde::encode_into_std_write(t, &mut dst, bincode::config::standard())
}

pub fn deserialize_from<T: DeserializeOwned, R: Read>(mut src: R) -> Result<T, DeserializeError> {
    bincode::serde::decode_from_std_read(&mut src, bincode::config::standard())
}

pub type SerializeError = bincode::error::EncodeError;

pub type DeserializeError = bincode::error::DecodeError;
