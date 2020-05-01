FROM rust:1.43

RUN CARGO_HOME=./.cargo \
    cargo install cargo-tarpaulin && \
    apt-get update && \
    apt-get install -y libgtk-3-dev xvfb