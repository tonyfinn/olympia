use crate::utils;

use std::fs;
use std::process;

#[test]
fn test_default() {
    let mut expected_output_path = utils::get_data_path();
    expected_output_path.push("fizzbuzz-disassembled.txt");
    let expected_output = fs::read(expected_output_path).expect("Failed reading test data");

    let mut input_file_path = utils::get_data_path();
    input_file_path.push("fizzbuzz.gb");

    let output = process::Command::new(utils::get_disassembler_bin())
        .arg(input_file_path)
        .output()
        .unwrap();

    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        String::from_utf8_lossy(&expected_output).replace("\r\n", "\n")
    );
}

#[test]
fn test_verbose() {
    let mut expected_output_path = utils::get_data_path();
    expected_output_path.push("fizzbuzz-disassembled-verbose.txt");
    let expected_output = fs::read(expected_output_path).expect("Failed reading test data");

    let mut input_file_path = utils::get_data_path();
    input_file_path.push("fizzbuzz.gb");

    let output = process::Command::new(utils::get_disassembler_bin())
        .arg("-v")
        .arg(input_file_path)
        .output()
        .unwrap();

    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        String::from_utf8_lossy(&expected_output).replace("\r\n", "\n")
    );
}
