use crate::utils;

use std::fs;
use std::process;

fn assert_disassembly_eql(output: String, expected: String) {
    for (line_idx, (output_line, expected_line)) in output.lines().zip(expected.lines()).enumerate()
    {
        assert_eq!(
            output_line,
            expected_line,
            "Expected {:?} but found {:?} on line {}",
            expected_line,
            output_line,
            line_idx + 1
        );
    }
    let output_lines = output.lines().count();
    let expected_lines = expected.lines().count();
    assert_eq!(
        expected_lines, output_lines,
        "Expected {} lines of disassembly but found {}",
        expected_lines, output_lines
    );
}

#[test]
fn test_default() {
    let mut expected_output_path = utils::get_data_path();
    expected_output_path.push("fizzbuzz-disassembled.txt");
    let expected_output = fs::read(expected_output_path).expect("Failed reading test data");

    let mut input_file_path = utils::get_data_path();
    input_file_path.push("fizzbuzz.gb");

    let output = process::Command::new(utils::get_cli_bin())
        .arg("disassemble")
        .arg(input_file_path)
        .output()
        .unwrap();

    assert_disassembly_eql(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        String::from_utf8_lossy(&expected_output).replace("\r\n", "\n"),
    );
}

#[test]
fn test_verbose() {
    let mut expected_output_path = utils::get_data_path();
    expected_output_path.push("fizzbuzz-disassembled-verbose.txt");
    let expected_output = fs::read(expected_output_path).expect("Failed reading test data");

    let mut input_file_path = utils::get_data_path();
    input_file_path.push("fizzbuzz.gb");

    let output = process::Command::new(utils::get_cli_bin())
        .arg("disassemble")
        .arg("-v")
        .arg(input_file_path)
        .output()
        .unwrap();

    assert_disassembly_eql(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        String::from_utf8_lossy(&expected_output).replace("\r\n", "\n"),
    );
}
