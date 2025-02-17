use crustcrab::{
    client::Client,
    server::{Server, ServerSender},
};
use std::thread;

fn main() {
    let (client_tx, client_rx) = crossbeam_channel::unbounded();
    let client = Client::new(client_tx);
    let server_tx = ServerSender::Proxy(client.create_proxy());
    let mut server = Server::new(server_tx, client_rx);
    thread::spawn(move || server.run());
    client.run();
}
