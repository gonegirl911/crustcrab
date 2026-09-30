use serde::{Serialize, de::DeserializeOwned};
use std::{
    io::{self, Read, Write},
    mem, slice,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Postcard(#[from] postcard::Error),
    #[error("connection closed at frame boundary")]
    ConnectionClosed,
}

pub fn send<T: Serialize, W: Write>(dst: &mut W, t: &T, buf: &mut Vec<u8>) -> Result<(), Error> {
    buf.clear();
    *buf = postcard::to_extend(t, mem::take(buf))?;
    let len = u32::try_from(buf.len()).unwrap();
    dst.write_all(&len.to_le_bytes())?;
    dst.write_all(buf)?;
    dst.flush()?;
    Ok(())
}

pub fn recv<T: DeserializeOwned, R: Read>(src: &mut R, buf: &mut Vec<u8>) -> Result<T, Error> {
    let mut header = [0; 4];
    if probe(src, &mut header[0])? == 0 {
        return Err(Error::ConnectionClosed);
    }
    src.read_exact(&mut header[1..])?;
    let len = u32::from_le_bytes(header) as usize;
    buf.clear();
    buf.reserve(len);
    let read = src.take(len as u64).read_to_end(buf)?;
    if read < len {
        return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
    }
    postcard::from_bytes(buf).map_err(Into::into)
}

fn probe<R: Read>(src: &mut R, byte: &mut u8) -> io::Result<usize> {
    loop {
        match src.read(slice::from_mut(byte)) {
            Ok(n) => break Ok(n),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => break Err(e),
        }
    }
}
