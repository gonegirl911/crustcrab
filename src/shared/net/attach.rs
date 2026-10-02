use super::{ConnectionSettings, codec};
use crate::shared::enum_map::{Enum, EnumMap, EnumMapBuilder};
use crossbeam_channel::{Receiver, RecvTimeoutError};
use log::{error, info, warn};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::{
    io, iter,
    net::{SocketAddr, TcpListener, TcpStream},
    process, thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

pub fn serve<R>(
    addrs: EnumMap<R, SocketAddr>,
    settings: ConnectionSettings,
) -> Receiver<EnumMap<R, (TcpStream, SocketAddr)>>
where
    R: Enum + Send + 'static,
{
    let listeners = addrs
        .into_iter()
        .map(|(role, addr)| match TcpListener::bind(addr) {
            Ok(listener) => {
                info!("[{addr}] create TCP listener");
                (role, addr, listener)
            }
            Err(e) => {
                error!("[{addr}] create TCP listener: {e}");
                process::exit(1);
            }
        })
        .collect::<Vec<_>>();

    let (arrive_tx, arrive_rx) = crossbeam_channel::unbounded();
    let (attached_tx, attached_rx) = crossbeam_channel::unbounded();

    for (role, addr, listener) in listeners {
        thread::spawn({
            let arrive_tx = arrive_tx.clone();
            move || {
                loop {
                    let (mut stream, addr) = match listener.accept() {
                        Ok((stream, peer_addr)) => {
                            info!("[{peer_addr}] open TCP connection to {addr}");
                            (stream, peer_addr)
                        }
                        Err(e) => {
                            error!("[{addr}] open TCP connection: {e}");
                            continue;
                        }
                    };
                    if let Err(e) = stream.set_nodelay(true) {
                        warn!("[{addr}] disable Nagle algorithm: {e}");
                    }
                    if let Err(e) = stream.set_read_timeout(Some(ATTACH_TIMEOUT)) {
                        warn!("[{addr}] set read timeout: {e}");
                    }

                    thread::spawn({
                        let arrive_tx = arrive_tx.clone();
                        move || {
                            let mut buf = Vec::new();
                            match codec::recv(&mut stream, &mut buf) {
                                Ok(Attach { token }) => {
                                    info!("[{addr}] attach {token}");
                                    _ = arrive_tx.send((token, role, stream, addr));
                                }
                                Err(codec::Error::Io(e))
                                    if matches!(
                                        e.kind(),
                                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                                    ) =>
                                {
                                    info!("[{addr}] timed out waiting for attach request");
                                }
                                Err(codec::Error::ConnectionClosed) => {
                                    info!("[{addr}] reading closed");
                                }
                                Err(e) => {
                                    error!("[{addr}] read attach request: {e}");
                                }
                            }
                        }
                    });
                }
            }
        });
    }

    thread::spawn(move || {
        let mut rendezvous = Rendezvous::<R, (TcpStream, SocketAddr)>::default();
        let mut buf = Vec::new();

        loop {
            let arrival = match rendezvous.next_deadline() {
                Some(deadline) => arrive_rx.recv_deadline(deadline),
                None => arrive_rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };

            let (token, role, stream, addr) = match arrival {
                Ok(arrival) => arrival,
                Err(RecvTimeoutError::Timeout) => {
                    let expired = rendezvous.evict_expired(Instant::now());
                    for token in expired {
                        warn!("[{token}] evict expired rendezvous");
                    }
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            };

            let mut completed = match rendezvous.arrive(token, role, (stream, addr)) {
                Arrival::Waiting => continue,
                Arrival::Replaced((_, replaced_addr)) => {
                    warn!("[{replaced_addr}] replaced by [{addr}]");
                    continue;
                }
                Arrival::Completed(completed) => completed,
            };

            let mut confirmed = true;
            for (stream, addr) in completed.values_mut() {
                if let Err(e) = codec::send(stream, &Attached { settings }, &mut buf) {
                    error!("[{addr}] write attach confirmation: {e}");
                    confirmed = false;
                    break;
                }
            }

            if confirmed {
                info!("[{token}] rendezvous complete");
                _ = attached_tx.send(completed);
            } else {
                error!("[{token}] drop unconfirmed rendezvous");
            }
        }
    });

    attached_rx
}

pub fn connect<const N: usize>(addrs: [SocketAddr; N]) -> ([TcpStream; N], ConnectionSettings) {
    let mut streams = Vec::with_capacity(N);
    let token = Uuid::new_v4();
    let mut buf = Vec::new();

    for addr in &addrs {
        let mut stream = match TcpStream::connect(addr) {
            Ok(stream) => {
                info!("[{addr}] open TCP connection");
                stream
            }
            Err(e) => {
                error!("[{addr}] open TCP connection: {e}");
                process::exit(1);
            }
        };
        if let Err(e) = stream.set_nodelay(true) {
            warn!("[{addr}] disable Nagle algorithm: {e}");
        }
        if let Err(e) = codec::send(&mut stream, &Attach { token }, &mut buf) {
            error!("[{addr}] write attach request: {e}");
            process::exit(1);
        }
        streams.push(stream);
    }

    let mut connection_settings = None;
    for (stream, addr) in iter::zip(&mut streams, &addrs) {
        match codec::recv(stream, &mut buf) {
            Ok(Attached { settings }) => {
                connection_settings = Some(settings);
            }
            Err(e) => {
                error!("[{addr}] read attach confirmation: {e}");
                process::exit(1);
            }
        }
    }
    info!("[{token}] attached");

    (streams.try_into().unwrap(), connection_settings.unwrap())
}

struct Rendezvous<R: Enum, S> {
    waiting: FxHashMap<Uuid, Waiting<R, S>>,
    timeout: Duration,
}

impl<R: Enum, S> Rendezvous<R, S> {
    fn arrive(&mut self, token: Uuid, role: R, stream: S) -> Arrival<R, S> {
        let mut waiting = self.waiting.remove(&token).unwrap_or_else(Waiting::new);
        let replaced = waiting.members.set(role, stream);
        match waiting.members.build() {
            Ok(completed) => Arrival::Completed(completed),
            Err(members) => {
                waiting.members = members;
                self.waiting.insert(token, waiting);
                match replaced {
                    Some(replaced) => Arrival::Replaced(replaced),
                    None => Arrival::Waiting,
                }
            }
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.waiting
            .values()
            .map(|waiting| waiting.since + self.timeout)
            .min()
    }

    fn evict_expired(&mut self, now: Instant) -> Vec<Uuid> {
        let expired = self.expired(now).collect::<Vec<_>>();
        for token in &expired {
            self.waiting.remove(token);
        }
        expired
    }

    fn expired(&self, now: Instant) -> impl Iterator<Item = Uuid> {
        self.waiting
            .iter()
            .filter(move |(_, waiting)| waiting.since + self.timeout <= now)
            .map(|(&token, _)| token)
    }
}

impl<R: Enum, S> Default for Rendezvous<R, S> {
    fn default() -> Self {
        Self {
            waiting: Default::default(),
            timeout: ATTACH_TIMEOUT,
        }
    }
}

struct Waiting<R: Enum, S> {
    members: EnumMapBuilder<R, S>,
    since: Instant,
}

impl<R: Enum, S> Waiting<R, S> {
    fn new() -> Self {
        Self {
            members: Default::default(),
            since: Instant::now(),
        }
    }
}

enum Arrival<R: Enum, S> {
    Waiting,
    Replaced(S),
    Completed(EnumMap<R, S>),
}

#[derive(Serialize, Deserialize)]
struct Attach {
    token: Uuid,
}

#[derive(Serialize, Deserialize)]
struct Attached {
    settings: ConnectionSettings,
}

const ATTACH_TIMEOUT: Duration = Duration::from_secs(30);
