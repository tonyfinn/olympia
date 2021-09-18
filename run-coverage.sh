#!/bin/bash
cargo tarpaulin --workspace --config .tarpaulin.toml \
    && xdg-open target/coverage/tarpaulin-report.html
