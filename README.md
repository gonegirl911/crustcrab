# Crustcrab

For a quick and dirty build, run:

```sh
cargo run --release
```

For a much faster and smaller binary at the cost of compilation speed, run:

```sh
RUST_LOG='crustcrab=info' \
    RUSTFLAGS='-C target-cpu=native' \
    cargo run --profile lto
```

This project also supports multiplayer. To host a server, run:

```sh
RUST_LOG='crustcrab=info' \
    RUSTFLAGS='-C target-cpu=native' \
    cargo run -p server --profile lto -- \
    --event-addr 0.0.0.0:8008 \
    --chunk-addr 0.0.0.0:8009
```

To connect a client, run:

```sh
RUST_LOG='crustcrab=info' \
    RUSTFLAGS='-C target-cpu=native' \
    cargo run -p client --profile lto -- \
    --event-addr 127.0.0.1:8008 \
    --chunk-addr 127.0.0.1:8009
```
