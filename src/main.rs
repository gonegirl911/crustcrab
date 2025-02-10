#![feature(
    array_chunks,
    array_windows,
    generic_arg_infer,
    impl_trait_in_assoc_type,
    int_roundings,
    let_chains,
    trait_alias,
    type_alias_impl_trait
)]

mod client;
mod server;
mod shared;

use client::Client;
use server::Server;
use std::thread;

fn main() {
    let (client_tx, client_rx) = crossbeam_channel::unbounded();
    let client = Client::new(client_tx);
    let server = Server::new(client.create_proxy(), client_rx);
    thread::spawn(move || server.run());
    client.run();
}
