//! Profile-aware repository commands over the shared cancellable add operation.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use devora_agent_space::Space;
use devora_config::config;
use devora_workspace::creation::{self, AddRepoTargetKind, CreationEvent, CreationHandle, Outcome};
use devora_workspace::workspace::{self, RepoInfo};

const HELP: &str = "Usage: debi repo list\n       debi repo add <repo> [--postfix <postfix>]\n\nList the current profile's repo names, or add a detached worktree to the current\nworkspace or Agent Space. DEBI_PROFILE_PATH selects the session profile first.\nAdd prints progress to stderr and the final absolute path to stdout.";

enum Failure {
    Usage(String),
    Operation(String),
}

pub fn run(args: &[OsString]) -> u8 {
    match execute(args) {
        Ok(code) => code,
        Err(Failure::Usage(message)) => {
            eprintln!("debi repo: {message}\n\n{HELP}");
            2
        }
        Err(Failure::Operation(message)) => {
            eprintln!("debi repo: {message}");
            1
        }
    }
}

enum Action<'a> {
    Help,
    List,
    Add {
        name: &'a str,
        postfix: Option<&'a str>,
    },
}

fn parse<'a>(args: &'a [OsString]) -> Result<Action<'a>, Failure> {
    let args = args
        .iter()
        .map(|arg| {
            arg.to_str()
                .ok_or_else(|| Failure::Usage("arguments must be UTF-8".into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    match args.as_slice() {
        [] | ["-h" | "--help" | "help"] | ["list" | "add", "-h" | "--help"] => Ok(Action::Help),
        ["list"] => Ok(Action::List),
        ["add", name] if !name.starts_with('-') => Ok(Action::Add {
            name,
            postfix: None,
        }),
        ["add", "--", name] => Ok(Action::Add {
            name,
            postfix: None,
        }),
        ["add", name, "--postfix", postfix] | ["add", "--postfix", postfix, name]
            if !name.starts_with('-') && !postfix.is_empty() =>
        {
            Ok(Action::Add {
                name,
                postfix: Some(postfix),
            })
        }
        _ => Err(Failure::Usage(
            "expected list or add <repo> [--postfix <postfix>]".into(),
        )),
    }
}

fn execute(args: &[OsString]) -> Result<u8, Failure> {
    let action = parse(args)?;
    if matches!(action, Action::Help) {
        println!("{HELP}");
        return Ok(0);
    }
    let profiles = workspace::list_profiles()
        .map_err(Failure::Operation)?
        .into_iter()
        .filter_map(|profile| Path::new(&profile.path).canonicalize().ok())
        .collect::<Vec<_>>();
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|path| path.canonicalize().ok());
    let target = match cwd {
        Some(cwd) => find_target(&profiles, &cwd)?,
        None => None,
    };
    let session_profile = std::env::var("DEBI_PROFILE_PATH")
        .ok()
        .and_then(|path| config::expand_tilde(&path).ok())
        .and_then(|path| Path::new(&path).canonicalize().ok())
        .filter(|path| profiles.contains(path));
    let profile = session_profile
        .or_else(|| target.as_ref().map(|target| target.profile.clone()))
        .ok_or_else(|| {
            Failure::Usage(
                "no profile found in DEBI_PROFILE_PATH or the current workspace/Agent Space".into(),
            )
        })?;
    let mut repos = workspace::get_registered_repos(utf8(&profile)?).map_err(Failure::Operation)?;
    repos.sort_by(|a, b| (&a.name, &a.path).cmp(&(&b.name, &b.path)));
    repos.dedup_by(|a, b| a.path == b.path);
    match action {
        Action::List => {
            for name in repos.iter().map(|repo| &repo.name).collect::<BTreeSet<_>>() {
                println!("{name}");
            }
            Ok(0)
        }
        Action::Add { name, postfix } => {
            let target = target.ok_or_else(|| Failure::Usage("the current directory is outside an initialized workspace or valid Agent Space".into()))?;
            if target.profile != profile {
                return Err(Failure::Usage(format!(
                    "session profile '{}' differs from the current target's owning profile '{}'",
                    profile.display(),
                    target.profile.display()
                )));
            }
            let repo = select_repo(&repos, name)?;
            let directory = match postfix {
                Some(postfix) => format!("{name}-{postfix}"),
                None => name.to_owned(),
            };
            creation::validate_worktree_name(&directory).map_err(Failure::Usage)?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| Failure::Operation(error.to_string()))?;
            runtime.block_on(add(target, repo.path.clone(), directory))
        }
        Action::Help => unreachable!(),
    }
}

struct Target {
    profile: PathBuf,
    path: PathBuf,
    kind: AddRepoTargetKind,
}

fn find_target(profiles: &[PathBuf], cwd: &Path) -> Result<Option<Target>, Failure> {
    for profile in profiles {
        for (folder, kind) in [
            ("workspaces", AddRepoTargetKind::Workspace),
            ("agent-spaces", AddRepoTargetKind::AgentSpace),
        ] {
            let root = profile.join(folder);
            let Ok(relative) = cwd.strip_prefix(&root) else {
                continue;
            };
            let Some(Component::Normal(name)) = relative.components().next() else {
                continue;
            };
            let path = root.join(name);
            let valid = match kind {
                AddRepoTargetKind::Workspace => path.is_dir() && path.join("initialized").exists(),
                AddRepoTargetKind::AgentSpace => Space::list(profile)
                    .map_err(Failure::Operation)?
                    .iter()
                    .any(|entry| {
                        entry.path == path && entry.identity.is_some() && entry.error.is_none()
                    }),
            };
            if valid {
                return Ok(Some(Target {
                    profile: profile.clone(),
                    path,
                    kind,
                }));
            }
        }
    }
    Ok(None)
}

fn select_repo<'a>(repos: &'a [RepoInfo], name: &str) -> Result<&'a RepoInfo, Failure> {
    let matches = repos
        .iter()
        .filter(|repo| repo.name == name)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [repo] => Ok(repo),
        [] => Err(Failure::Usage(format!(
            "repo '{name}' is unavailable in this profile"
        ))),
        _ => Err(Failure::Usage(format!(
            "repo '{name}' is ambiguous. Candidate paths:\n{}",
            matches
                .iter()
                .map(|repo| repo.path.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        ))),
    }
}

fn utf8(path: &Path) -> Result<&str, Failure> {
    path.to_str()
        .ok_or_else(|| Failure::Operation(format!("path is not UTF-8: {}", path.display())))
}

async fn add(target: Target, source: String, directory: String) -> Result<u8, Failure> {
    // Install the handler before starting work, then await rollback after Ctrl+C.
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .map_err(|error| Failure::Operation(error.to_string()))?;
    let handle = CreationHandle::new();
    let worker_handle = handle.clone();
    let destination = target.path.join(&directory);
    let path = utf8(&target.path)?.to_owned();
    let mut worker = tokio::task::spawn_blocking(move || {
        let events: creation::EventSink = Arc::new(|event| match event {
            CreationEvent::Step { label } => eprintln!("{label}"),
            CreationEvent::Log { line } => eprintln!("{line}"),
            _ => {}
        });
        let mut warnings = vec![];
        let result = creation::add_repo_with_kind(
            &worker_handle,
            &path,
            &source,
            &directory,
            target.kind,
            &events,
            &mut warnings,
        );
        for warning in warnings {
            eprintln!("Warning: {warning}");
        }
        result
    });
    let result = tokio::select! {
        result = &mut worker => result,
        _ = interrupt.recv() => {
            handle.request_cancel();
            worker.await
        }
    }
    .map_err(|error| Failure::Operation(error.to_string()))?
    .map_err(Failure::Operation)?;
    match result {
        Outcome::Done(_) => {
            println!("{}", destination.display());
            Ok(0)
        }
        Outcome::Cancelled => {
            if destination
                .try_exists()
                .map_err(|error| Failure::Operation(error.to_string()))?
            {
                return Err(Failure::Operation(format!(
                    "cancelled, but rollback left '{}' for inspection",
                    destination.display()
                )));
            }
            eprintln!("Cancelled. The added worktree was rolled back.");
            Ok(130)
        }
    }
}
