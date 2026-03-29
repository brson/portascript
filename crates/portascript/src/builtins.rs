use std::ffi::OsString;
use std::process::{Command, Stdio};

/// All known builtin names.
const BUILTINS: &[&str] = &[
    "echo", "cat", "true", "false", "ls", "cp", "sort", "head", "tail",
    "mkdir", "rm", "mv", "chmod", "wc", "basename", "dirname", "touch",
    "tr", "cut", "tee", "uniq", "seq", "yes", "printf",
];

/// Check if a name is a known builtin.
pub fn is_builtin(name: &str) -> bool {
    BUILTINS.contains(&name)
}

/// Run a builtin by name via self-recursive subprocess.
///
/// Inherits stdin/stdout/stderr from the parent process (passthrough).
pub fn run_builtin(name: &str, args: Vec<String>) -> Option<i32> {
    if !is_builtin(name) {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let status = Command::new(exe)
        .arg("--internal-builtin")
        .arg(name)
        .args(&args)
        .status()
        .ok()?;
    Some(status.code().unwrap_or(1))
}

/// Run a builtin, capturing stdout. Optionally feeds stdin data.
pub fn run_builtin_capture(name: &str, args: Vec<String>, stdin_data: Option<&str>) -> Option<(i32, String)> {
    if !is_builtin(name) {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let mut cmd = Command::new(exe);
    cmd.arg("--internal-builtin")
        .arg(name)
        .args(&args)
        .stdout(Stdio::piped());

    if stdin_data.is_some() {
        cmd.stdin(Stdio::piped());
    }

    let mut child = cmd.spawn().ok()?;

    if let Some(data) = stdin_data {
        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(data.as_bytes()).ok()?;
        }
    }

    let output = child.wait_with_output().ok()?;
    let code = output.status.code().unwrap_or(1);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    Some((code, stdout))
}

/// Run a uutils builtin directly in-process.
///
/// Called from the `--internal-builtin` CLI mode.
pub fn run_direct(name: &str, args: &[String]) -> i32 {
    let os_args: Vec<OsString> = args.iter().map(|s| OsString::from(s)).collect();
    match name {
        "echo" => uu_echo::uumain(os_args.into_iter()),
        "cat" => uu_cat::uumain(os_args.into_iter()),
        "true" => uu_true::uumain(os_args.into_iter()),
        "false" => uu_false::uumain(os_args.into_iter()),
        "ls" => uu_ls::uumain(os_args.into_iter()),
        "cp" => uu_cp::uumain(os_args.into_iter()),
        "sort" => uu_sort::uumain(os_args.into_iter()),
        "head" => uu_head::uumain(os_args.into_iter()),
        "tail" => uu_tail::uumain(os_args.into_iter()),
        "mkdir" => uu_mkdir::uumain(os_args.into_iter()),
        "rm" => uu_rm::uumain(os_args.into_iter()),
        "mv" => uu_mv::uumain(os_args.into_iter()),
        "chmod" => uu_chmod::uumain(os_args.into_iter()),
        "wc" => uu_wc::uumain(os_args.into_iter()),
        "basename" => uu_basename::uumain(os_args.into_iter()),
        "dirname" => uu_dirname::uumain(os_args.into_iter()),
        "touch" => uu_touch::uumain(os_args.into_iter()),
        "tr" => uu_tr::uumain(os_args.into_iter()),
        "cut" => uu_cut::uumain(os_args.into_iter()),
        "tee" => uu_tee::uumain(os_args.into_iter()),
        "uniq" => uu_uniq::uumain(os_args.into_iter()),
        "seq" => uu_seq::uumain(os_args.into_iter()),
        "yes" => uu_yes::uumain(os_args.into_iter()),
        "printf" => uu_printf::uumain(os_args.into_iter()),
        _ => {
            eprintln!("portascript: unknown builtin '{}'", name);
            1
        }
    }
}
