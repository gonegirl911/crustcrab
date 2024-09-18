use crate::{client::Client, server::Server};
use std::thread;

pub struct App {
    client: Client,
    server: Server,
}

impl App {
    pub fn new() -> Self {
        let (client_tx, client_rx) = crossbeam_channel::unbounded();
        let client = Client::new(client_tx);
        let server = Server::new(client.create_proxy(), client_rx);
        Self { client, server }
    }

    pub fn run(self) {
        thread::spawn(move || self.server.run());
        self.client.run();
    }
}
