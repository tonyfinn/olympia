#!/bin/sh
# test threads 1 needed because of tarpaulin segfault
# https://github.com/xd009642/tarpaulin/issues/190
cargo tarpaulin -o Html -- --test-threads 1
