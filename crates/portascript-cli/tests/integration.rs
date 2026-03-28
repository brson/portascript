use std::path::{Path, PathBuf};
use std::process::Command;

#[allow(unused_imports)]
use tempfile;

struct ScriptResult {
    stdout: String,
    stderr: String,
    code: i32,
}

fn scripts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/scripts")
}

fn run_script(name: &str) -> ScriptResult {
    let bin = env!("CARGO_BIN_EXE_portascript");
    let script = scripts_dir().join(name);
    let output = Command::new(bin)
        .arg(&script)
        .output()
        .expect("failed to run portascript");
    ScriptResult {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        code: output.status.code().unwrap_or(-1),
    }
}

fn run_script_with_args(name: &str, args: &[&str]) -> ScriptResult {
    let bin = env!("CARGO_BIN_EXE_portascript");
    let script = scripts_dir().join(name);
    let output = Command::new(bin)
        .arg(&script)
        .args(args)
        .output()
        .expect("failed to run portascript");
    ScriptResult {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        code: output.status.code().unwrap_or(-1),
    }
}

fn run_script_in_dir(name: &str, dir: &Path) -> ScriptResult {
    let bin = env!("CARGO_BIN_EXE_portascript");
    let script = scripts_dir().join(name);
    let output = Command::new(bin)
        .arg(&script)
        .current_dir(dir)
        .output()
        .expect("failed to run portascript");
    ScriptResult {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        code: output.status.code().unwrap_or(-1),
    }
}

// --- Step 1: Empty script ---

#[test]
fn test_001_empty() {
    let r = run_script("001_empty.psc");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, "");
    assert_eq!(r.stderr, "");
}

// --- Step 2: Comment ---

#[test]
fn test_002_comment() {
    let r = run_script("002_comment.psc");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, "");
    assert_eq!(r.stderr, "");
}

// --- Step 3: Whitespace/blank lines ---

#[test]
fn test_003_whitespace() {
    let r = run_script("003_whitespace.psc");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, "");
    assert_eq!(r.stderr, "");
}

// --- Step 4: print("hello") ---

#[test]
fn test_004_print_string() {
    let r = run_script("004_print_string.psc");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, "hello");
    assert_eq!(r.stderr, "");
}

// --- Step 5: print numbers ---

#[test]
fn test_005_print_numbers() {
    let r = run_script("005_print_numbers.psc");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, "423.14");
}

// --- Step 6: print booleans ---

#[test]
fn test_006_print_bools() {
    let r = run_script("006_print_bools.psc");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, "truefalse");
}

// --- Step 7: let binding ---

#[test]
fn test_007_let_binding() {
    let r = run_script("007_let_binding.psc");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, "world");
}

// --- Step 8: let mut + reassignment ---

#[test]
fn test_008_let_mut() {
    let r = run_script("008_let_mut.psc");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, "2");
}

// --- Step 9: immutable reassignment error ---

#[test]
fn test_009_immutable_error() {
    let r = run_script("009_immutable_error.psc");
    assert_ne!(r.code, 0);
    assert!(r.stderr.contains("immutable"), "stderr: {}", r.stderr);
}

// --- Step 10: arithmetic ---

#[test]
fn test_010_arithmetic() {
    let r = run_script("010_arithmetic.psc");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, "163");
}

// --- Step 11: string concatenation ---

#[test]
fn test_011_string_concat() {
    let r = run_script("011_string_concat.psc");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, "hello world");
}

// --- Step 12: string interpolation ---

#[test]
fn test_012_interpolation() {
    let r = run_script("012_interpolation.psc");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, "hello world, number 42");
}

// --- Step 13: single-quoted raw strings ---

#[test]
fn test_013_raw_string() {
    let r = run_script("013_raw_string.psc");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, "hello {name}");
}

// --- Step 14: eprintln ---

#[test]
fn test_014_eprintln() {
    let r = run_script("014_eprintln.psc");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, "");
    assert_eq!(r.stderr, "error message\n");
}

// --- Step 15: run echo ---

#[test]
fn test_015_run_echo() {
    let r = run_script("015_run_echo.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "hello world\n");
}

// --- Step 16: run echo with interpolation ---

#[test]
fn test_016_run_echo_interp() {
    let r = run_script("016_run_echo_interp.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "hello world\n");
}

// --- Step 17: run echo with expression arg ---

#[test]
fn test_017_run_echo_expr() {
    let r = run_script("017_run_echo_expr.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "42\n");
}

// --- Step 18: exec ---

#[test]
fn test_018_exec() {
    let r = run_script("018_exec.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "hello from exec\n");
}

// --- Step 19: exec nonzero aborts ---

#[test]
fn test_019_exec_fail() {
    let r = run_script("019_exec_fail.psc");
    assert_ne!(r.code, 0);
    assert!(!r.stdout.contains("should not reach here"), "stdout: {}", r.stdout);
}

// --- Step 20: $() capture with exec ---

#[test]
fn test_020_capture() {
    let r = run_script("020_capture.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "captured");
}

// --- Step 21: $() capture with run ---

#[test]
fn test_021_capture_run() {
    let r = run_script("021_capture_run.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "from builtin");
}

// --- Step 22: if/else ---

#[test]
fn test_022_if_else() {
    let r = run_script("022_if_else.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "big");
}

// --- Step 23: elif ---

#[test]
fn test_023_elif() {
    let r = run_script("023_elif.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "zero");
}

// --- Step 24: logical operators ---

#[test]
fn test_024_logical() {
    let r = run_script("024_logical.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "in range ok");
}

// --- Step 25: while loop ---

#[test]
fn test_025_while() {
    let r = run_script("025_while.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "012");
}

// --- Step 26: for/in list ---

#[test]
fn test_026_for_list() {
    let r = run_script("026_for_list.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "abc");
}

// --- Step 27: break/continue ---

#[test]
fn test_027_break_continue() {
    let r = run_script("027_break_continue.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "124");
}

// --- Step 28: match ---

#[test]
fn test_028_match() {
    let r = run_script("028_match.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "rust");
}

// --- Step 29: function ---

#[test]
fn test_029_function() {
    let r = run_script("029_function.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "hello world");
}

// --- Step 30: function return ---

#[test]
fn test_030_fn_return() {
    let r = run_script("030_fn_return.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "7");
}

// --- Step 31: ? error suppression ---

#[test]
fn test_031_question_mark() {
    let r = run_script("031_question_mark.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "survived");
}

// --- Step 32: try ---

#[test]
fn test_032_try() {
    let r = run_script("032_try.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "false1");
}

// --- Step 33: try stdout ---

#[test]
fn test_033_try_stdout() {
    let r = run_script("033_try_stdout.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "hello\n");
}

// --- Step 34: pipeline ---

#[test]
fn test_034_pipeline() {
    let r = run_script("034_pipeline.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "hello\n");
}

// --- Step 35: pipeline capture ---

#[test]
fn test_035_pipeline_capture() {
    let r = run_script("035_pipeline_capture.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "hello world");
}

// --- Step 36: run cat ---

#[test]
fn test_036_run_cat() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("test.txt"), "hello").unwrap();
    let r = run_script_in_dir("036_run_cat.psc", dir.path());
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "hello");
}

// --- Step 37: run true/false ---

#[test]
fn test_037_run_true_false() {
    let r = run_script("037_run_true_false.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "false");
}

// --- Step 38: string builtins ---

#[test]
fn test_038_string_builtins() {
    let r = run_script("038_string_builtins.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "11a-b-c");
}

// --- Step 39: more string builtins ---

#[test]
fn test_039_more_string_builtins() {
    let r = run_script("039_more_string_builtins.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "3trueHELLO");
}

// --- Step 40: type conversion ---

#[test]
fn test_040_type_conversion() {
    let r = run_script("040_type_conversion.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "433.14int");
}

// --- Step 41: list ops ---

#[test]
fn test_041_list_ops() {
    let r = run_script("041_list_ops.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "a34");
}

// --- Step 42: map ops ---

#[test]
fn test_042_map_ops() {
    let r = run_script("042_map_ops.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "alicetrue2name,age");
}

// --- Step 43: map mutation ---

#[test]
fn test_043_map_mut() {
    let r = run_script("043_map_mut.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "2");
}

// --- Step 44: range ---

#[test]
fn test_044_range() {
    let r = run_script("044_range.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "6");
}

// --- Step 45: env ---

#[test]
fn test_045_env() {
    let r = run_script("045_env.psc");
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "strdefault");
}

// --- Step 46: exit ---

#[test]
fn test_046_exit() {
    let r = run_script("046_exit.psc");
    assert_eq!(r.code, 42);
    assert_eq!(r.stdout, "before");
}

// --- Step 47: error ---

#[test]
fn test_047_error() {
    let r = run_script("047_error.psc");
    assert_ne!(r.code, 0);
    assert!(r.stderr.contains("something went wrong"), "stderr: {}", r.stderr);
}

// --- Step 48: args ---

#[test]
fn test_048_args() {
    let r = run_script_with_args("048_args.psc", &["hello"]);
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert_eq!(r.stdout, "2hello");
}
