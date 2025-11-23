use crustcrab::{client::Client, server::Server};
use std::thread;

fn main() {
    let (client_tx, client_rx) = crossbeam_channel::unbounded();
    let (client, server_tx) = Client::new(client_tx);
    let mut server = Server::new(server_tx, client_rx);
    thread::spawn(move || server.run());
    client.run();
}
