use std::ffi::OsString;
use std::io::Read;

fn make_args(name: &str, args: Vec<String>) -> Vec<OsString> {
    std::iter::once(OsString::from(name))
        .chain(args.into_iter().map(OsString::from))
        .collect()
}

fn dispatch(name: &str, os_args: Vec<OsString>) -> Option<i32> {
    match name {
        "echo" => Some(uu_echo::uumain(os_args.into_iter())),
        "cat" => Some(uu_cat::uumain(os_args.into_iter())),
        "true" => Some(uu_true::uumain(os_args.into_iter())),
        "false" => Some(uu_false::uumain(os_args.into_iter())),
        _ => None,
    }
}

/// Run a uutils builtin by name, writing to process stdout/stderr.
pub fn run_builtin(name: &str, args: Vec<String>) -> Option<i32> {
    dispatch(name, make_args(name, args))
}

/// Run a uutils builtin, capturing its stdout.
pub fn run_builtin_capture(name: &str, args: Vec<String>) -> Option<(i32, String)> {
    let os_args = make_args(name, args);

    let (mut reader, writer) = os_pipe::pipe().ok()?;
    let writer_fd = writer.try_clone().ok()?;

    let code = {
        let _guard = gag::Redirect::stdout(writer).ok()?;
        drop(writer_fd);
        let code = dispatch(name, os_args)?;
        code
    };

    let mut output = String::new();
    reader.read_to_string(&mut output).ok()?;
    Some((code, output))
}
