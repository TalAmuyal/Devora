//! `debi`, Devora's CLI.
//!
//! Commands ported to Rust run here: `repo` and `ccc`, also reachable as a `ccc` symlink to this binary.
//! Every other command is handed to the Go `debi-go` by `exec`, so its arguments, stdio, exit code and signals pass through untouched.
//! Command groups move here one at a time, and `debi-go` is retired once none is left (ADR-007, TalAmuyal/Devora#210).

use std::ffi::OsString;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus};

use devora_agent::Ccc;
use devora_config::config::DEFAULT_CLAUDE_COMMAND;

mod repo;

/// The Go CLI that runs every command not yet ported.
const DEBI_GO: &str = "debi-go";

/// The session's Claude Code command (`devora_config`'s `command` setting), as its terminal exports it.
const COMMAND_ENV: &str = "DEVORA_CCC_COMMAND";

/// What a shell reports for a command it cannot run.
const EXIT_CANNOT_RUN: u8 = 127;

fn main() -> ExitCode {
    let mut argv = std::env::args_os();
    let invoked_as = argv.next().unwrap_or_default();
    let args: Vec<OsString> = argv.collect();

    if Path::new(&invoked_as)
        .file_name()
        .is_some_and(|name| name == "ccc")
    {
        return ccc(&args);
    }
    match args.split_first() {
        Some((command, rest)) if command == "ccc" => ccc(rest),
        Some((command, rest)) if command == "repo" => ExitCode::from(repo::run(rest)),
        _ => delegate(&args),
    }
}

/// The directory this binary lives in, with symlinks resolved, so a `ccc` symlink finds the bundle it belongs to.
fn own_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    exe.parent().map(Path::to_path_buf)
}

fn ccc(args: &[OsString]) -> ExitCode {
    // In the app bundle: `Resources/bundled-apps/debi` next to `Resources/cc-plugins/`.
    let plugins_dir =
        own_dir().and_then(|dir| dir.parent().map(|parent| parent.join("cc-plugins")));
    let ccc = devora_agent::ccc(args, plugins_dir.as_deref(), |name| {
        std::env::var(name).ok()
    });
    // The session's Claude Code command, which its terminal exports; a terminal from before the setting has none.
    let command = std::env::var(COMMAND_ENV)
        .ok()
        .filter(|command| !command.is_empty())
        .unwrap_or_else(|| DEFAULT_CLAUDE_COMMAND.to_owned());
    let Some(program) =
        devora_agent::program::expand(&command, std::env::var("HOME").ok().as_deref())
    else {
        eprintln!("ccc: cannot run {command}: there is no home directory (HOME) for ~/");
        return ExitCode::from(EXIT_CANNOT_RUN);
    };
    match ccc {
        Ccc::Update => match Command::new(&program).arg("--update").status() {
            Ok(status) => exit_code(status),
            Err(error) => cannot_run(&command, &error),
        },
        Ccc::Launch(spec) => {
            let error = Command::new(&program)
                .args(&spec.args)
                .envs(spec.env)
                .exec();
            cannot_run(&command, &error)
        }
    }
}

fn delegate(args: &[OsString]) -> ExitCode {
    let sibling = own_dir()
        .map(|dir| dir.join(DEBI_GO))
        .filter(|path| path.is_file());
    let program = sibling.map_or_else(|| OsString::from(DEBI_GO), PathBuf::into_os_string);
    let error = Command::new(&program).args(args).exec();
    cannot_run(DEBI_GO, &error)
}

/// A child's exit status as a shell reports it: its code, or 128 + the signal that killed it.
fn exit_code(status: ExitStatus) -> ExitCode {
    let code = status
        .code()
        .or_else(|| status.signal().map(|signal| 128 + signal));
    ExitCode::from(code.and_then(|c| u8::try_from(c).ok()).unwrap_or(1))
}

fn cannot_run(program: &str, error: &std::io::Error) -> ExitCode {
    eprintln!("debi: cannot run {program}: {error}");
    ExitCode::from(EXIT_CANNOT_RUN)
}
