use std::ffi::OsString;
use std::io::Read;

/// Run a uutils builtin by name, writing to process stdout/stderr.
///
/// Returns the exit code, or None if the builtin is unknown.
pub fn run_builtin(name: &str, args: Vec<String>) -> Option<i32> {
    let os_args: Vec<OsString> = std::iter::once(OsString::from(name))
        .chain(args.into_iter().map(OsString::from))
        .collect();

    match name {
        "echo" => Some(uu_echo::uumain(os_args.into_iter())),
        _ => None,
    }
}

/// Run a uutils builtin, capturing its stdout.
///
/// Returns (exit_code, captured_stdout), or None if unknown.
pub fn run_builtin_capture(name: &str, args: Vec<String>) -> Option<(i32, String)> {
    let os_args: Vec<OsString> = std::iter::once(OsString::from(name))
        .chain(args.into_iter().map(OsString::from))
        .collect();

    // Redirect stdout through an OS pipe.
    let (mut reader, writer) = os_pipe::pipe().ok()?;
    let writer_fd = writer.try_clone().ok()?;

    let code = {
        // Redirect stdout for the duration of the builtin call.
        let _guard = gag::Redirect::stdout(writer).ok()?;
        drop(writer_fd);
        let code = match name {
            "echo" => uu_echo::uumain(os_args.into_iter()),
            _ => return None,
        };
        // Guard drops here, restoring stdout.
        code
    };

    let mut output = String::new();
    reader.read_to_string(&mut output).ok()?;
    Some((code, output))
}
