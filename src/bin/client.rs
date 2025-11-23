use clap::Parser;
use crustcrab::{
    client::{Client, ClientEvent},
    shared::bincode,
};
use std::{
    io::{self, BufReader, BufWriter, Write},
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
    let (client_tx, client_rx) = crossbeam_channel::unbounded();
    let (client, server_tx) = Client::new(client_tx.clone());

    let Args {
        priority_addr,
        addr,
    } = Parser::parse();
    let priority_stream = match TcpStream::connect(&priority_addr) {
        Ok(stream) => {
            eprintln!("[{priority_addr}] open TCP connection SUCCEDED");
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
            eprintln!("[{addr}] open TCP connection SUCCEDED");
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
            loop {
                let event = match bincode::deserialize_from(&mut priority_reader) {
                    Ok(event) => event,
                    Err(bincode::DeserializeError::Io { inner, .. })
                        if inner.kind() == io::ErrorKind::UnexpectedEof =>
                    {
                        break;
                    }
                    Err(e) => {
                        eprintln!("[{priority_addr}] read server event FAILED: {e}");
                        continue;
                    }
                };
                if server_tx.send([event]).is_err() {
                    break;
                }
            }
            _ = client_tx.send(ClientEvent::ServerDisconnected);
            eprintln!("[{priority_addr}] reading CLOSED");
        });

        s.spawn(|| {
            let mut priority_writer = BufWriter::new(&priority_stream);
            for event in client_rx {
                if matches!(event, ClientEvent::ServerDisconnected) {
                    break;
                }
                if let Err(e) = bincode::serialize_into(event, &mut priority_writer) {
                    if let bincode::SerializeError::Io { inner, .. } = &e
                        && inner.kind() == io::ErrorKind::BrokenPipe
                    {
                        break;
                    }
                    eprintln!("[{priority_addr}] write client event FAILED: {e}");
                    continue;
                }
                if let Err(e) = priority_writer.flush() {
                    if e.kind() == io::ErrorKind::BrokenPipe {
                        break;
                    }
                    eprintln!("[{priority_addr}] write client event FAILED: {e}");
                }
            }
            eprintln!("[{priority_addr}] writing CLOSED");
        });

        s.spawn(|| {
            let mut reader = BufReader::new(&stream);
            loop {
                let event = match bincode::deserialize_from(&mut reader) {
                    Ok(event) => event,
                    Err(bincode::DeserializeError::Io { inner, .. })
                        if inner.kind() == io::ErrorKind::UnexpectedEof =>
                    {
                        break;
                    }
                    Err(e) => {
                        eprintln!("[{addr}] read server event FAILED: {e}");
                        continue;
                    }
                };
                if server_tx.send([event]).is_err() {
                    break;
                }
            }
            eprintln!("[{addr}] reading CLOSED");
        });

        client.run();

        if let Err(e) = priority_stream.shutdown(Shutdown::Both)
            && e.kind() != io::ErrorKind::NotConnected
        {
            eprintln!("[{priority_addr}] gracefull shutdown FAILED: {e}");
        }
        if let Err(e) = stream.shutdown(Shutdown::Both)
            && e.kind() != io::ErrorKind::NotConnected
        {
            eprintln!("[{addr}] gracefull shutdown FAILED: {e}");
        }
    });
}
