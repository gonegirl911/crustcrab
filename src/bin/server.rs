#![feature(let_chains)]

use clap::Parser;
use crustcrab::{
    client::ClientEvent,
    server::{Server, ServerEvent, ServerSender},
};
use std::{
    io::{self, BufReader, BufWriter, Write},
    net::TcpListener,
    thread,
};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value_t = 8008)]
    priority_port: u16,
    #[arg(long, default_value_t = 8009)]
    port: u16,
}

fn main() {
    let (client_tx, client_rx) = crossbeam_channel::unbounded();
    let mut server = Server::new(ServerSender::disconnected(), client_rx);

    thread::spawn(move || {
        let args = Args::parse();
        let priority_addr = format!("127.0.0.1:{}", args.priority_port);
        let priority_listener = match TcpListener::bind(&priority_addr) {
            Ok(listener) => {
                eprintln!("[{priority_addr}] create TCP listener SUCCEDED");
                listener
            }
            Err(e) => {
                eprintln!("[{priority_addr}] create TCP listener FAILED: {e}");
                return;
            }
        };
        let addr = format!("127.0.0.1:{}", args.port);
        let listener = match TcpListener::bind(&addr) {
            Ok(listener) => {
                eprintln!("[{addr}] create TCP listener SUCCEDED");
                listener
            }
            Err(e) => {
                eprintln!("[{addr}] create TCP listener FAILED: {e}");
                return;
            }
        };

        for (priority_stream, stream) in priority_listener.incoming().zip(listener.incoming()) {
            let priority_stream = match priority_stream {
                Ok(stream) => {
                    eprintln!("[{priority_addr}] open TCP connection SUCCEDED");
                    stream
                }
                Err(e) => {
                    eprintln!("[{priority_addr}] open TCP connection FAILED: {e}");
                    continue;
                }
            };
            if let Err(e) = priority_stream.set_nodelay(true) {
                eprintln!("[{priority_addr}] disable Nagle algorithm FAILED: {e}");
            }
            let stream = match stream {
                Ok(stream) => {
                    eprintln!("[{addr}] open TCP connection SUCCEDED");
                    stream
                }
                Err(e) => {
                    eprintln!("[{addr}] open TCP connection FAILED: {e}");
                    continue;
                }
            };
            if let Err(e) = stream.set_nodelay(true) {
                eprintln!("[{addr}] disable Nagle algorithm FAILED: {e}");
            }

            let (priority_server_tx, priority_server_rx) = crossbeam_channel::unbounded();
            let (server_tx, server_rx) = crossbeam_channel::unbounded();
            let server_tx = ServerSender::Sender {
                priority_tx: priority_server_tx,
                tx: server_tx,
            };
            client_tx
                .send(ClientEvent::Connected(server_tx.clone().into()))
                .unwrap_or_else(|_| unreachable!());

            thread::scope(|s| {
                s.spawn(|| {
                    let mut priority_writer = BufWriter::new(&priority_stream);
                    for event in &priority_server_rx {
                        if matches!(event, ServerEvent::ClientDisconnected) {
                            break;
                        }
                        if let Err(e) = bincode::serialize_into(&mut priority_writer, &event) {
                            if let bincode::ErrorKind::Io(e) = &*e
                                && e.kind() == io::ErrorKind::BrokenPipe
                            {
                                break;
                            }
                            eprintln!("[{priority_addr}] write server event FAILED: {e}");
                            continue;
                        }
                        if let Err(e) = priority_writer.flush() {
                            if e.kind() == io::ErrorKind::BrokenPipe {
                                break;
                            }
                            eprintln!("[{priority_addr}] write server event FAILED: {e}");
                        }
                    }
                    eprintln!("[{priority_addr}] writing CLOSED");
                });

                s.spawn(|| {
                    let mut writer = BufWriter::new(&stream);
                    for event in &server_rx {
                        if matches!(event, ServerEvent::ClientDisconnected) {
                            break;
                        }
                        if let Err(e) = bincode::serialize_into(&mut writer, &event) {
                            if let bincode::ErrorKind::Io(e) = &*e
                                && e.kind() == io::ErrorKind::BrokenPipe
                            {
                                break;
                            }
                            eprintln!("[{addr}] write server event FAILED: {e}");
                            continue;
                        }
                        if let Err(e) = writer.flush() {
                            if e.kind() == io::ErrorKind::BrokenPipe {
                                break;
                            }
                            eprintln!("[{addr}] write server event FAILED: {e}");
                        }
                    }
                    eprintln!("[{addr}] writing CLOSED");
                });

                let mut priority_reader = BufReader::new(&priority_stream);
                loop {
                    let event = match bincode::deserialize_from(&mut priority_reader) {
                        Ok(event) => event,
                        Err(e) => {
                            if let bincode::ErrorKind::Io(e) = &*e
                                && let io::ErrorKind::ConnectionReset | io::ErrorKind::UnexpectedEof =
                                    e.kind()
                            {
                                server_tx
                                    .send(ServerEvent::ClientDisconnected)
                                    .unwrap_or_else(|_| unreachable!());
                                break;
                            }
                            eprintln!("[{priority_addr}] read client event FAILED: {e}");
                            continue;
                        }
                    };
                    if client_tx.send(event).is_err() {
                        break;
                    }
                }
                eprintln!("[{priority_addr}] reading CLOSED");
            });
        }
    });

    server.run();
}
