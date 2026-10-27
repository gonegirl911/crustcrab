pub mod connection;
pub mod net;

use crossbeam_channel::Receiver;
use domain::protocol::player;

pub fn run(
    connection_rx: Receiver<connection::Event>,
    player_rx: Receiver<(connection::Id, player::Event)>,
) {
    loop {}
}
