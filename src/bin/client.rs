use clap::Parser;
use crustcrab::{
    client::Client,
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
    control_addr: String,
    #[arg(long, default_value = "localhost:8009")]
    chunk_addr: String,
}

fn main() {
    pool::init(1);

    let (player_tx, player_rx) = crossbeam_channel::unbounded();
    let (control_tx, control_rx) = crossbeam_channel::unbounded();
    let (chunk_tx, chunk_rx) = crossbeam_channel::unbounded();

    let client = Client::new(player_tx, control_rx, chunk_rx);

    let Args {
        control_addr,
        chunk_addr,
    } = Parser::parse();
    let control_stream = match TcpStream::connect(&control_addr) {
        Ok(stream) => {
            eprintln!("[{control_addr}] open TCP connection SUCCEEDED");
            stream
        }
        Err(e) => {
            eprintln!("[{control_addr}] open TCP connection FAILED: {e}");
            return;
        }
    };
    if let Err(e) = control_stream.set_nodelay(true) {
        eprintln!("[{control_addr}] disable Nagle algorithm FAILED: {e}");
    }
    let chunk_stream = match TcpStream::connect(&chunk_addr) {
        Ok(stream) => {
            eprintln!("[{chunk_addr}] open TCP connection SUCCEEDED");
            stream
        }
        Err(e) => {
            eprintln!("[{chunk_addr}] open TCP connection FAILED: {e}");
            return;
        }
    };
    if let Err(e) = chunk_stream.set_nodelay(true) {
        eprintln!("[{chunk_addr}] disable Nagle algorithm FAILED: {e}");
    }

    thread::scope(|s| {
        s.spawn(|| {
            let mut control_reader = BufReader::new(&control_stream);
            let mut buf = Vec::new();
            loop {
                let event = match codec::recv(&mut control_reader, &mut buf) {
                    Ok(event) => event,
                    Err(codec::Error::ConnectionClosed) => break,
                    Err(e) => {
                        eprintln!("[{control_addr}] read server event FAILED: {e}");
                        break;
                    }
                };
                if control_tx.send(event).is_err() {
                    break;
                }
            }
            eprintln!("[{control_addr}] reading CLOSED");
        });

        s.spawn(|| {
            let mut control_writer = BufWriter::new(&control_stream);
            let mut buf = Vec::new();
            for event in player_rx {
                if let Err(e) = codec::send(&mut control_writer, &event, &mut buf) {
                    eprintln!("[{control_addr}] write client event FAILED: {e}");
                    break;
                }
            }
            eprintln!("[{control_addr}] writing CLOSED");
        });

        s.spawn(|| {
            let mut chunk_reader = BufReader::new(&chunk_stream);
            let mut buf = Vec::new();
            loop {
                let event = match codec::recv(&mut chunk_reader, &mut buf) {
                    Ok(event) => event,
                    Err(codec::Error::ConnectionClosed) => break,
                    Err(e) => {
                        eprintln!("[{chunk_addr}] read server event FAILED: {e}");
                        break;
                    }
                };
                if chunk_tx.send(event).is_err() {
                    break;
                }
            }
            eprintln!("[{chunk_addr}] reading CLOSED");
        });

        client.run();

        if let Err(e) = control_stream.shutdown(Shutdown::Both)
            && e.kind() != io::ErrorKind::NotConnected
        {
            eprintln!("[{control_addr}] graceful shutdown FAILED: {e}");
        }
        if let Err(e) = chunk_stream.shutdown(Shutdown::Both)
            && e.kind() != io::ErrorKind::NotConnected
        {
            eprintln!("[{chunk_addr}] graceful shutdown FAILED: {e}");
        }
    });
}
