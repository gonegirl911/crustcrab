mod app;
mod client;
mod math;
mod server;

fn main() -> ! {
    pollster::block_on(app::App::new()).run()
}
