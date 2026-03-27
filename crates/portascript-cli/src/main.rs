use rmx::std::path::PathBuf;

use rmx::clap::{self, Parser as _};

fn main() {
    let cli = Cli::parse();

    let source = match rmx::std::fs::read_to_string(&cli.script) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("portascript: {}: {}", cli.script.display(), e);
            std::process::exit(1);
        }
    };

    let mut args: Vec<String> = vec![cli.script.display().to_string()];
    args.extend(cli.args);

    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();

    match portascript::interpret(&source, args, &mut stdout, &mut stderr) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            // Flush stdout before writing error to stderr.
            use std::io::Write;
            let _ = stdout.flush();
            drop(stdout);
            eprintln!("portascript: {}", e);
            std::process::exit(1);
        }
    }
}

#[derive(clap::Parser)]
#[command(name = "portascript")]
struct Cli {
    /// Script file to execute.
    script: PathBuf,

    /// Arguments passed to the script.
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
}
