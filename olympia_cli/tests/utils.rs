use std::env;
use std::path::PathBuf;

pub(crate) fn find_crate_root() -> PathBuf {
    // Why not use CARGO_MANIFEST_DIR?
    // cargo-tarpaulin doesn't pass it through to tests
    // when run for coverage.
    let mut path = env::current_exe().unwrap(); // target/debug/deps/test_module
    while !path.ends_with("target") {
        path.pop();
    }
    path.pop();
    path
}

pub(crate) fn get_data_path() -> PathBuf {
    let mut path = find_crate_root();
    path.push("olympia_cli"); // olympia_cli
    path.push("tests");
    path.push("data");
    path
}

pub(crate) fn get_cli_bin() -> PathBuf {
    let path_str: &'static str = env!("CARGO_BIN_EXE_olympia_cli");
    let mut path = PathBuf::new();
    path.push(path_str);
    path
}
