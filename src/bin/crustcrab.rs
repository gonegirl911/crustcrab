use crustcrab::{
    client::{
        Client,
        net::{self, DisconnectReason},
    },
    server::{
        Server,
        connection::{Connection, ConnectionEvent, ConnectionId},
    },
    shared::pool,
};
use std::thread;

fn main() {
    env_logger::init();
    pool::init(2);

    let (connection_tx, connection_rx) = crossbeam_channel::bounded(0);
    let (player_tx, forward_rx) = crossbeam_channel::unbounded();
    let (forward_tx, player_rx) = crossbeam_channel::unbounded();
    let (control_tx, relay_rx) = crossbeam_channel::unbounded();
    let (relay_tx, control_rx) = crossbeam_channel::unbounded();
    let (chunk_tx, chunk_rx) = crossbeam_channel::unbounded();
    let (disconnect_tx, disconnect_rx) = crossbeam_channel::unbounded();

    let mut server = Server::new(connection_rx, player_rx);
    thread::spawn(move || server.run());

    let id = ConnectionId::new();
    connection_tx
        .send(ConnectionEvent::Opened(
            id,
            Connection {
                control_tx,
                chunk_tx,
            },
        ))
        .unwrap();

    thread::spawn(move || {
        for event in forward_rx {
            _ = forward_tx.send((id, event));
        }
    });

    thread::spawn({
        let player_tx = player_tx.clone();
        move || {
            let disconnect_reason = loop {
                let Ok(event) = relay_rx.recv() else {
                    break DisconnectReason::Closed;
                };

                if let Err(reason) = net::relay(event, &player_tx, &relay_tx) {
                    break reason;
                }
            };
            let _ = disconnect_tx.send(disconnect_reason);
        }
    });

    let client = Client::new(player_tx, control_rx, chunk_rx, disconnect_rx);
    client.run();
}
