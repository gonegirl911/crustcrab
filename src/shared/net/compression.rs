use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, DeserializeOwned},
};
use std::{
    cell::RefCell,
    io, mem,
    sync::{Arc, OnceLock},
};
use zstd::{
    bulk::{Compressor, Decompressor},
    zstd_safe,
};

#[derive(Clone)]
pub struct Compressed<T> {
    pub inner: T,
    wire: Arc<OnceLock<Box<[u8]>>>,
}

impl<T> From<T> for Compressed<T> {
    fn from(inner: T) -> Self {
        Self {
            inner,
            wire: Default::default(),
        }
    }
}

impl<T: Serialize> Serialize for Compressed<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let wire = self.wire.get_or_init(|| {
            RAW_SCRATCH
                .with_borrow_mut(|buf| {
                    buf.clear();
                    *buf = postcard::to_extend(&self.inner, mem::take(buf)).unwrap();
                    COMPRESSOR.with_borrow_mut(|ctx| compress(ctx, buf))
                })
                .into()
        });
        serializer.serialize_bytes(wire)
    }
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for Compressed<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = Deserialize::deserialize(deserializer)?;
        let raw = DECOMPRESSOR
            .with_borrow_mut(|ctx| decompress(ctx, wire))
            .map_err(de::Error::custom)?;
        Ok(Self {
            inner: postcard::from_bytes(&raw).map_err(de::Error::custom)?,
            wire: Default::default(),
        })
    }
}

fn compress(ctx: &mut Compressor, raw: &[u8]) -> Vec<u8> {
    ctx.compress(raw).unwrap()
}

fn decompress(ctx: &mut Decompressor, wire: &[u8]) -> io::Result<Vec<u8>> {
    let len = zstd_safe::get_frame_content_size(wire)
        .map_err(|e| io::Error::other(e.to_string()))?
        .ok_or_else(|| io::Error::other("zstd frame lacks content size"))?;

    if len > MAX_CONTENT_LEN as u64 {
        return Err(io::Error::other(format!(
            "zstd content size {len} exceeds {MAX_CONTENT_LEN}"
        )));
    }

    ctx.decompress(wire, len as usize)
}

thread_local! {
    static COMPRESSOR: RefCell<Compressor<'static>> = Compressor::new(1).unwrap().into();
    static DECOMPRESSOR: RefCell<Decompressor<'static>> = Decompressor::new().unwrap().into();
    static RAW_SCRATCH: RefCell<Vec<u8>> = const { RefCell::new(vec![]) };
}

const MAX_CONTENT_LEN: usize = 256 * 1024;
