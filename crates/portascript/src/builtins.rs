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
        "ls" => Some(uu_ls::uumain(os_args.into_iter())),
        "cp" => Some(uu_cp::uumain(os_args.into_iter())),
        "sort" => Some(uu_sort::uumain(os_args.into_iter())),
        "head" => Some(uu_head::uumain(os_args.into_iter())),
        "tail" => Some(uu_tail::uumain(os_args.into_iter())),
        "mkdir" => Some(uu_mkdir::uumain(os_args.into_iter())),
        "rm" => Some(uu_rm::uumain(os_args.into_iter())),
        "mv" => Some(uu_mv::uumain(os_args.into_iter())),
        "chmod" => Some(uu_chmod::uumain(os_args.into_iter())),
        "wc" => Some(uu_wc::uumain(os_args.into_iter())),
        "basename" => Some(uu_basename::uumain(os_args.into_iter())),
        "dirname" => Some(uu_dirname::uumain(os_args.into_iter())),
        "touch" => Some(uu_touch::uumain(os_args.into_iter())),
        "tr" => Some(uu_tr::uumain(os_args.into_iter())),
        "cut" => Some(uu_cut::uumain(os_args.into_iter())),
        "tee" => Some(uu_tee::uumain(os_args.into_iter())),
        "uniq" => Some(uu_uniq::uumain(os_args.into_iter())),
        "seq" => Some(uu_seq::uumain(os_args.into_iter())),
        "yes" => Some(uu_yes::uumain(os_args.into_iter())),
        "printf" => Some(uu_printf::uumain(os_args.into_iter())),
        _ => None,
    }
}

/// Run a uutils builtin by name, writing to process stdout/stderr.
pub fn run_builtin(name: &str, args: Vec<String>) -> Option<i32> {
    dispatch(name, make_args(name, args))
}

/// Run a uutils builtin, capturing stdout. Optionally feeds stdin data.
pub fn run_builtin_capture(name: &str, args: Vec<String>, stdin_data: Option<&str>) -> Option<(i32, String)> {
    let os_args = make_args(name, args);

    // Set up stdout capture pipe.
    let (mut stdout_reader, stdout_writer) = os_pipe::pipe().ok()?;
    let stdout_writer_clone = stdout_writer.try_clone().ok()?;

    // Set up stdin pipe if needed.
    let stdin_setup = if let Some(data) = stdin_data {
        use std::io::Write;
        let (stdin_reader, mut stdin_writer) = os_pipe::pipe().ok()?;
        stdin_writer.write_all(data.as_bytes()).ok()?;
        drop(stdin_writer);
        Some(stdin_reader)
    } else {
        None
    };

    let code = {
        // Redirect stdin if we have data.
        let _stdin_guard = stdin_setup.map(|reader| redirect_stdin(reader));
        let _stdout_guard = gag::Redirect::stdout(stdout_writer).ok()?;
        drop(stdout_writer_clone);
        dispatch(name, os_args)?
    };

    let mut output = String::new();
    stdout_reader.read_to_string(&mut output).ok()?;
    Some((code, output))
}

/// Redirect process stdin to read from the given file.
/// Returns a guard that restores stdin on drop.
fn redirect_stdin(file: os_pipe::PipeReader) -> StdinRedirectGuard {
    use std::os::unix::io::AsRawFd;

    let stdin_fd = 0;
    // Save the original stdin fd.
    let saved_fd = unsafe { libc::dup(stdin_fd) };
    // Replace stdin with our pipe reader.
    unsafe { libc::dup2(file.as_raw_fd(), stdin_fd) };
    drop(file);

    StdinRedirectGuard { saved_fd }
}

/// RAII guard that restores stdin on drop.
struct StdinRedirectGuard {
    saved_fd: i32,
}

impl Drop for StdinRedirectGuard {
    fn drop(&mut self) {
        let stdin_fd = 0;
        unsafe {
            libc::dup2(self.saved_fd, stdin_fd);
            libc::close(self.saved_fd);
        }
    }
}
