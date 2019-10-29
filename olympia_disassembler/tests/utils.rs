use std::env;
use std::path::PathBuf;

pub(crate) fn get_data_path() -> PathBuf {
    // Why not use CARGO_MANIFEST_DIR?
    // cargo-tarpaulin doesn't pass it through to tests
    // when run for coverage.
    let mut path = env::current_exe().unwrap(); // target/debug/deps/test_module
    path.pop(); // target/debug/deps
    path.pop(); // target/debug
    path.pop(); // target
    path.pop(); // <crate root>
    path.push("olympia_disassembler"); // olympia_disassembler
    path.push("tests");
    path.push("data");
    path
}

pub(crate) fn get_disassembler_bin() -> PathBuf {
    let mut path = env::current_exe().unwrap(); // target/debug/deps/test_module
    path.pop(); // target/debug/deps
    path.pop(); // target/debug
    path.push("olympia_disassembler");
    path
}
