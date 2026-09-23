use clap::Parser;
use crustcrab::{
    client::{Client, ClientEvent},
    shared::{codec, pool},
};
use std::{
    io::{self, BufReader, BufWriter},
    net::{Shutdown, TcpStream},
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
    pool::init(1);

    let (client_tx, client_rx) = crossbeam_channel::unbounded();
    let (client, server_tx) = Client::new(client_tx.clone());

    let Args {
        priority_addr,
        addr,
    } = Parser::parse();
    let priority_stream = match TcpStream::connect(&priority_addr) {
        Ok(stream) => {
            eprintln!("[{priority_addr}] open TCP connection SUCCEEDED");
            stream
        }
        Err(e) => {
            eprintln!("[{priority_addr}] open TCP connection FAILED: {e}");
            return;
        }
    };
    if let Err(e) = priority_stream.set_nodelay(true) {
        eprintln!("[{priority_addr}] disable Nagle algorithm FAILED: {e}");
    }
    let stream = match TcpStream::connect(&addr) {
        Ok(stream) => {
            eprintln!("[{addr}] open TCP connection SUCCEEDED");
            stream
        }
        Err(e) => {
            eprintln!("[{addr}] open TCP connection FAILED: {e}");
            return;
        }
    };
    if let Err(e) = stream.set_nodelay(true) {
        eprintln!("[{addr}] disable Nagle algorithm FAILED: {e}");
    }

    thread::scope(|s| {
        s.spawn(|| {
            let mut priority_reader = BufReader::new(&priority_stream);
            let mut buf = Vec::new();
            loop {
                let event = match codec::recv(&mut priority_reader, &mut buf) {
                    Ok(event) => event,
                    Err(codec::Error::ConnectionClosed) => break,
                    Err(e) => {
                        eprintln!("[{priority_addr}] read server event FAILED: {e}");
                        break;
                    }
                };
                if server_tx.send(event).is_err() {
                    break;
                }
            }
            _ = client_tx.send(ClientEvent::ServerDisconnected);
            eprintln!("[{priority_addr}] reading CLOSED");
        });

        s.spawn(|| {
            let mut priority_writer = BufWriter::new(&priority_stream);
            let mut buf = Vec::new();
            for event in client_rx {
                if matches!(event, ClientEvent::ServerDisconnected) {
                    break;
                }
                if let Err(e) = codec::send(&mut priority_writer, &event, &mut buf) {
                    eprintln!("[{priority_addr}] write client event FAILED: {e}");
                    break;
                }
            }
            eprintln!("[{priority_addr}] writing CLOSED");
        });

        s.spawn(|| {
            let mut reader = BufReader::new(&stream);
            let mut buf = Vec::new();
            loop {
                let event = match codec::recv(&mut reader, &mut buf) {
                    Ok(event) => event,
                    Err(codec::Error::ConnectionClosed) => break,
                    Err(e) => {
                        eprintln!("[{addr}] read server event FAILED: {e}");
                        break;
                    }
                };
                if server_tx.send(event).is_err() {
                    break;
                }
            }
            eprintln!("[{addr}] reading CLOSED");
        });

        client.run();

        if let Err(e) = priority_stream.shutdown(Shutdown::Both)
            && e.kind() != io::ErrorKind::NotConnected
        {
            eprintln!("[{priority_addr}] graceful shutdown FAILED: {e}");
        }
        if let Err(e) = stream.shutdown(Shutdown::Both)
            && e.kind() != io::ErrorKind::NotConnected
        {
            eprintln!("[{addr}] graceful shutdown FAILED: {e}");
        }
    });
}
