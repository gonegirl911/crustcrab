#![feature(
    array_chunks,
    generic_arg_infer,
    impl_trait_in_assoc_type,
    int_roundings,
    let_chains,
    trait_alias,
    type_alias_impl_trait
)]

mod app;
mod client;
mod server;
mod shared;

fn main() {
    app::App::new().run();
}
