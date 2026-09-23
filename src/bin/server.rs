use clap::Parser;
use crustcrab::{
    client::ClientEvent,
    server::{Server, ServerEvent, ServerSender},
    shared::{codec, pool},
};
use std::{
    io::{BufReader, BufWriter},
    net::TcpListener,
    thread,
};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "localhost:8008")]
    priority_addr: String,
    #[arg(long, default_value = "localhost:8009")]
    addr: String,
}

fn main() {
    pool::init(2);

    let (client_tx, client_rx) = crossbeam_channel::unbounded();
    let mut server = Server::new(ServerSender::Disconnected, client_rx);

    thread::spawn(move || {
        let Args {
            priority_addr,
            addr,
        } = Parser::parse();
        let priority_listener = match TcpListener::bind(&priority_addr) {
            Ok(listener) => {
                eprintln!("[{priority_addr}] create TCP listener SUCCEEDED");
                listener
            }
            Err(e) => {
                eprintln!("[{priority_addr}] create TCP listener FAILED: {e}");
                return;
            }
        };
        let listener = match TcpListener::bind(&addr) {
            Ok(listener) => {
                eprintln!("[{addr}] create TCP listener SUCCEEDED");
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
                    eprintln!("[{priority_addr}] open TCP connection SUCCEEDED");
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
                    eprintln!("[{addr}] open TCP connection SUCCEEDED");
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
            client_tx
                .send(ClientEvent::Connected(
                    ServerSender::Sender {
                        priority_tx: priority_server_tx.clone(),
                        tx: server_tx.clone(),
                    }
                    .into(),
                ))
                .unwrap();

            thread::scope(|s| {
                s.spawn(|| {
                    let mut priority_writer = BufWriter::new(&priority_stream);
                    let mut buf = Vec::new();
                    for event in priority_server_rx {
                        if matches!(event, ServerEvent::ClientDisconnected) {
                            break;
                        }
                        if let Err(e) = codec::send(&mut priority_writer, &event, &mut buf) {
                            eprintln!("[{priority_addr}] write server event FAILED: {e}");
                            break;
                        }
                    }
                    eprintln!("[{priority_addr}] writing CLOSED");
                });

                s.spawn(|| {
                    let mut writer = BufWriter::new(&stream);
                    let mut buf = Vec::new();
                    for event in server_rx {
                        if matches!(event, ServerEvent::ClientDisconnected) {
                            break;
                        }
                        if let Err(e) = codec::send(&mut writer, &event, &mut buf) {
                            eprintln!("[{addr}] write server event FAILED: {e}");
                            break;
                        }
                    }
                    eprintln!("[{addr}] writing CLOSED");
                });

                let mut priority_reader = BufReader::new(&priority_stream);
                let mut buf = Vec::new();
                loop {
                    let event = match codec::recv(&mut priority_reader, &mut buf) {
                        Ok(event) => event,
                        Err(codec::Error::ConnectionClosed) => break,
                        Err(e) => {
                            eprintln!("[{priority_addr}] read client event FAILED: {e}");
                            break;
                        }
                    };
                    client_tx.send(event).unwrap();
                }
                eprintln!("[{priority_addr}] reading CLOSED");

                _ = priority_server_tx.send(ServerEvent::ClientDisconnected);
                _ = server_tx.send(ServerEvent::ClientDisconnected);
            });
        }
    });

    server.run();
}
