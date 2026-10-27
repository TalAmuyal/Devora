use std::io::{self, Read, Write};
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::process::ExitCode;

use judgess::store::Store;
use judgess::{classify, export, hook};

/// Relative to the working directory, which is `project-judgess/` for the mise tasks.
const DEFAULT_CASES_FILE: &str = "cases.json";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("hook") => run_hook(),
        Some("classify") => user_command(|store, _| {
            classify::run(store, &mut io::stdin().lock(), &mut io::stdout())
        }),
        Some("export") => match cases_file(&args[1..]) {
            Some(cases_file) => {
                user_command(|store, home| export::run(store, home, &cases_file, &mut io::stdout()))
            }
            None => usage(),
        },
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!("usage: judgess hook | classify | export [--cases <cases.json>]");
    ExitCode::from(2)
}

/// The target of `export`: `--cases <path>`, or the default when there are no arguments at all.
fn cases_file(args: &[String]) -> Option<PathBuf> {
    match args {
        [] => Some(DEFAULT_CASES_FILE.into()),
        [flag, path] if flag == "--cases" => Some(path.into()),
        _ => None,
    }
}

/// A hook failure must never reach the agent: whatever goes wrong, print nothing and exit 0.
fn run_hook() -> ExitCode {
    panic::set_hook(Box::new(|_| {}));
    let _ = panic::catch_unwind(AssertUnwindSafe(|| {
        let mut input = String::new();
        if io::stdin().read_to_string(&mut input).is_err() {
            return;
        }
        let Some(home) = home_dir() else { return };
        if let Some(output) = hook::run(&input, &Store::in_home(&home)) {
            let _ = writeln!(io::stdout(), "{output}");
        }
    }));
    ExitCode::SUCCESS
}

fn user_command(run: impl FnOnce(&Store, Option<&str>) -> io::Result<()>) -> ExitCode {
    let Some(home) = home_dir() else {
        eprintln!("HOME is not set");
        return ExitCode::FAILURE;
    };
    match run(&Store::in_home(&home), Some(&home)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn home_dir() -> Option<String> {
    std::env::var("HOME").ok().filter(|home| !home.is_empty())
}
