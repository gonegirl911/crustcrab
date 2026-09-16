use crustcrab::{client::Client, server::Server, shared::pool};
use std::thread;

fn main() {
    pool::init(3);

    let (client_tx, client_rx) = crossbeam_channel::unbounded();
    let (client, server_tx) = Client::new(client_tx);
    let mut server = Server::new(server_tx, client_rx);
    thread::spawn(move || server.run());
    client.run();
}
