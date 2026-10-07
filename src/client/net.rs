use super::PlayerEvent;
use crate::server::{ChunkEvent, ControlEvent};
use crate::shared::net::{KEEP_ALIVE_TIMEOUT, attach, codec};
use crossbeam_channel::{Receiver, Sender};
use log::{error, info, warn};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::{
    io::{BufReader, BufWriter},
    thread,
};

pub fn connect(
    event_addr: SocketAddr,
    chunk_addr: SocketAddr,
) -> (
    Sender<PlayerEvent>,
    Receiver<ControlEvent>,
    Receiver<ChunkEvent>,
) {
    let [event_stream, chunk_stream] = attach::connect([event_addr, chunk_addr]);
    let event_stream = Arc::new(event_stream);
    let chunk_stream = Arc::new(chunk_stream);

    if let Err(e) = event_stream.set_read_timeout(Some(READ_TIMEOUT)) {
        warn!("[{event_addr}] set read timeout: {e}");
    }
    if let Err(e) = event_stream.set_write_timeout(Some(WRITE_TIMEOUT)) {
        warn!("[{event_addr}] set write timeout: {e}");
    }

    let (player_tx, player_rx) = crossbeam_channel::unbounded();
    let (control_tx, control_rx) = crossbeam_channel::unbounded();
    let (chunk_tx, chunk_rx) = crossbeam_channel::unbounded();

    thread::spawn({
        let event_stream = event_stream.clone();
        let player_tx = player_tx.clone();
        move || {
            let mut control_reader = BufReader::new(&*event_stream);
            let mut buf = Vec::new();
            loop {
                let event = match codec::recv(&mut control_reader, &mut buf) {
                    Ok(event) => event,
                    Err(codec::Error::ConnectionClosed) => break,
                    Err(codec::Error::Io(e))
                        if matches!(
                            e.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) =>
                    {
                        info!("[{event_addr}] server stopped responding");
                        break;
                    }
                    Err(e) => {
                        error!("[{event_addr}] read control event: {e}");
                        break;
                    }
                };

                match &event {
                    &ControlEvent::KeepAlive { tag } => {
                        if player_tx.send(PlayerEvent::KeepAlive { tag }).is_err() {
                            break;
                        }
                        continue;
                    }
                    ControlEvent::Kicked { reason } => {
                        error!("[{event_addr}] kicked: {reason}");
                    }
                    _ => {}
                }

                if control_tx.send(event).is_err() {
                    break;
                }
            }
            info!("[{event_addr}] reading closed");
        }
    });

    thread::spawn(move || {
        let mut player_writer = BufWriter::new(&*event_stream);
        let mut buf = Vec::new();
        for event in player_rx {
            match codec::send(&mut player_writer, &event, &mut buf) {
                Ok(()) => {}
                Err(codec::Error::Io(e))
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    info!("[{event_addr}] server stopped responding");
                    break;
                }
                Err(e) => {
                    error!("[{event_addr}] write player event: {e}");
                    break;
                }
            }
        }
        info!("[{event_addr}] writing closed");
    });

    thread::spawn(move || {
        let mut chunk_reader = BufReader::new(&*chunk_stream);
        let mut buf = Vec::new();
        loop {
            let event = match codec::recv(&mut chunk_reader, &mut buf) {
                Ok(event) => event,
                Err(codec::Error::ConnectionClosed) => break,
                Err(e) => {
                    error!("[{chunk_addr}] read chunk event: {e}");
                    break;
                }
            };
            if chunk_tx.send(event).is_err() {
                break;
            }
        }
        info!("[{chunk_addr}] reading closed");
    });

    (player_tx, control_rx, chunk_rx)
}

const READ_TIMEOUT: Duration = KEEP_ALIVE_TIMEOUT;
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);
