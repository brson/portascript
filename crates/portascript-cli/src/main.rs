use rmx::std::path::PathBuf;

use rmx::clap::{self, Parser as _};

fn main() {
    // Fast path: internal builtin invocation (self-recursive subprocess).
    // Usage: portascript --internal-builtin <name> [args...]
    let raw_args: Vec<String> = std::env::args().collect();
    if raw_args.len() >= 3 && raw_args[1] == "--internal-builtin" {
        let name = &raw_args[2];
        let builtin_args = &raw_args[2..]; // name + args (uumain expects argv[0] = name)
        let code = portascript::run_builtin_direct(name, builtin_args);
        std::process::exit(code);
    }

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
