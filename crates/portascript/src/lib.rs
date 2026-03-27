mod token;
mod value;
mod scope;
mod exec;

use rmx::prelude::*;

/// A portascript runtime error.
#[derive(Debug)]
pub struct PsError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl std::fmt::Display for PsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error at line {} col {}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for PsError {}

/// Interpret a portascript source string.
///
/// Returns the exit code.
pub fn interpret(
    source: &str,
    _args: Vec<String>,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
) -> AnyResult<i32> {
    let mut executor = exec::Executor::new(stdout, stderr);
    let code = executor.run(source)?;
    Ok(code)
}
