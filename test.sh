#!/bin/bash
cargo build
cargo test -- --skip gtk_
cargo test -- --test-threads 1 gtk_