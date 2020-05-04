FROM rust:1.43-slim

RUN CARGO_HOME=/builds/tonyfinn/olympia/.cargo \
    mkdir -p $CARGO_HOME && \
    apt-get update && \
    apt-get install -y libgtk-3-dev xvfb pkg-config libssl-dev && \
    cargo install --git https://github.com/xd009642/tarpaulin.git --branch develop