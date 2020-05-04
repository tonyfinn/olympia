FROM rust:1.43

RUN CARGO_HOME=./.cargo \
    cargo install --git https://github.com/xd009642/tarpaulin.git --branch develop && \
    apt-get update && \
    apt-get install -y libgtk-3-dev xvfb