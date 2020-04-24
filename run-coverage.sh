#!/bin/bash
cargo tarpaulin --config .tarpaulin.toml \
    && xdg-open target/coverage/tarpaulin-report.html
