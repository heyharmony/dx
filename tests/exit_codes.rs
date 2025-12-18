use std::process::Command;

fn run_dx(args: &[&str]) -> i32 {
    let exe = env!("CARGO_BIN_EXE_dx");
    let status = Command::new(exe).args(args).status().expect("run dx");
    status.code().unwrap_or(1)
}

#[test]
fn aliases_exits_zero() {
    // Running with no menu should still print a hint and exit 0 when asking for aliases
    let code = run_dx(&["aliases"]);
    assert_eq!(code, 0);
}

#[test]
fn missing_file_exits_one() {
    let code = run_dx(&["this-file-does-not-exist-123456.txt"]);
    assert_eq!(code, 1);
}
