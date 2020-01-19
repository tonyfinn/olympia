use crate::utils;

use std::io;
use std::io::Write;
use std::process;
use std::time::Duration;

fn execute_command(child: &mut process::Child, cmd: &str) -> io::Result<()> {
    let stdin = child.stdin.as_mut().unwrap();
    stdin.write_fmt(format_args!("{}\n", cmd))?;
    stdin.flush()?;

    Ok(())
}

fn run_debugging_session(cmds: &[&str]) -> io::Result<(Vec<String>, Vec<String>)> {
    let mut input_file_path = utils::get_data_path();
    input_file_path.push("fizzbuzz.gb");

    let mut child = process::Command::new(utils::get_cli_bin())
        .arg("debug")
        .arg(input_file_path)
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::piped())
        .spawn()?;

    std::thread::sleep(Duration::from_millis(200));

    for cmd in cmds.iter().chain(&["exit"]) {
        execute_command(&mut child, cmd)?;
    }

    child.wait_with_output().map(|result| {
        let output: Vec<String> = String::from_utf8_lossy(&result.stdout)
            .lines()
            .map(|s| s.into())
            .collect();
        let errors: Vec<String> = String::from_utf8_lossy(&result.stderr)
            .lines()
            .map(|s| s.into())
            .collect();
        (output, errors)
    })
}

#[test]
fn debugger_integration() {
    let mut input_file_path = utils::get_data_path();
    input_file_path.push("fizzbuzz.gb");

    let (actual_output_lines, actual_error_lines) =
        run_debugging_session(&["br pc 0x150", "ff", "r a"]).unwrap();

    let expected_error_lines: Vec<String> = vec![];
    let expected_output_lines: Vec<String> = vec![
        "Added breakpoint for register PC == 150".into(),
        "Broke on Breakpoint: register PC == 150".into(),
        "1".into(),
        "Exiting".into(),
    ];

    assert_eq!(
        actual_error_lines, expected_error_lines,
        "Expected no errors but found {:?}",
        actual_error_lines
    );
    assert_eq!(actual_output_lines, expected_output_lines);
}
