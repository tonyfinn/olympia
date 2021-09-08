FROM rust:1.54-slim

RUN CARGO_HOME=/builds/tonyfinn/olympia/.cargo \
    mkdir -p $CARGO_HOME && \
    apt-get update && \
    apt-get install --no-install-recommends -y libgtk-3-dev xvfb pkg-config libssl-dev git && \
    cargo install --git https://github.com/xd009642/tarpaulin.git --branch develop
