use base::rayon;
use client::{
    Client,
    net::{self, disconnect},
};
use server::connection::{self, Connection};
use std::thread;

fn main() {
    env_logger::init();
    rayon::global::init(2);

    let client = Client::new();

    let (connection_tx, connection_rx) = crossbeam_channel::bounded(1);
    let (player_tx, player_forward_rx) = crossbeam_channel::unbounded();
    let (player_forward_tx, player_rx) = crossbeam_channel::unbounded();
    let (control_tx, control_relay_rx) = crossbeam_channel::unbounded();
    let (control_relay_tx, control_rx) = crossbeam_channel::unbounded();
    let (chunk_tx, chunk_rx) = crossbeam_channel::unbounded();
    let (disconnect_tx, disconnect_rx) = disconnect::channel(client.create_proxy());

    let id = connection::Id::new();
    connection_tx
        .send(connection::Event::Opened(
            id,
            Connection {
                control_tx,
                chunk_tx,
            },
        ))
        .unwrap();

    thread::spawn(move || {
        for event in player_forward_rx {
            if player_forward_tx.send((id, event)).is_err() {
                break;
            }
        }
    });

    thread::spawn({
        let player_tx = player_tx.clone();
        move || {
            let disconnect_reason = loop {
                let Ok(event) = control_relay_rx.recv() else {
                    break disconnect::Reason::Closed;
                };

                if let Err(reason) = net::relay(event, &player_tx, &control_relay_tx) {
                    break reason;
                }
            };
            let _ = disconnect_tx.send(disconnect_reason);
        }
    });

    thread::spawn(move || server::run(connection_rx, player_rx));
    client.run(player_tx, control_rx, chunk_rx, disconnect_rx);
}
