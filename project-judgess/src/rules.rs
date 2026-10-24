//! The rule tables.
//! Every literal here is also part of the redaction vocabulary (see `vocabulary`), so a case that exercises a rule keeps the tokens the rule depends on.

use crate::model::RuleId;

pub(crate) const SHELLS: &[&str] = &["bash", "sh", "zsh"];

/// Commands that run a transparent wrapper around another command.
pub(crate) const WRAPPERS: &[&str] = &[
    "timeout", "time", "nice", "env", "command", "xargs", "watch",
];

pub(crate) const DENY_PREFIXES: &[(&[&str], RuleId)] = &[
    (&["eval"], RuleId::Eval),
    (&["git", "add"], RuleId::GitAddViaPrSkill),
];

/// Denied with a redirect to mise unless already running under `mise exec`.
pub(crate) const USE_MISE_COMMANDS: &[&str] = &[
    "cargo",
    "go",
    "node",
    "bun",
    "npm",
    "npx",
    "tsx",
    "poetry",
    "shellcheck",
    "helm",
    "python",
    "python3",
    "ruff",
];

pub(crate) const USE_MISE_EXEMPT_PREFIXES: &[&[&str]] = &[&["python", "-c"], &["python3", "-c"]];

pub(crate) const KNOWN_PREFIXES: &[(&[&str], RuleId)] = &[
    (&["rm"], RuleId::IrreversibleRm),
    (&["chmod"], RuleId::Chmod),
    (&["sudo"], RuleId::PrivilegeEscalation),
    (&["doas"], RuleId::PrivilegeEscalation),
    (&["git", "checkout"], RuleId::IrreversibleGitCheckout),
    (&["git", "reset"], RuleId::IrreversibleGitReset),
];

pub(crate) const APPROVED_COMMANDS: &[&str] = &[
    "cat",
    "echo",
    "mise",
    "test",
    "[",
    "awk",
    "find",
    "basename",
    "ls",
    "tree",
    "tr",
    "printf",
    "which",
    "make",
    "cp",
    "mkdir",
    "wc",
    "nm",
    "otool",
    "true",
    "false",
    ":",
    "head",
    "tail",
    "stat",
    "tee",
    "sort",
    "uniq",
    "grep",
    "rg",
    "sed",
    "col",
    "hash",
    "open",
    "gaac",
    "submit-pr",
    "man",
    ".venv/bin/python",
    "cd",
    "curl",
    "wget",
    "diff",
];

pub(crate) const APPROVED_PREFIXES: &[&[&str]] = &[
    &["command", "-v"],
    &["bash", "-n"],
    &["git", "diff"],
    &["git", "grep"],
    &["git", "show"],
    &["git", "log"],
    &["git", "ls-files"],
    &["git", "status"],
    &["git", "ls-tree"],
    &["git", "fetch"],
    &["git", "rm"],
    &["git", "mv"],
    &["git", "rev-parse"],
    &["git", "check-ignore"],
    &["git", "help"],
    &["git", "init"],
    &["git", "stash", "list"],
    &["git", "notes", "list"],
    &["git", "remote", "get-url"],
    &["debi", "git", "ls-files"],
    &["debi", "git", "status"],
    &["debi", "git", "ls-tree"],
    &["debi", "git", "fetch"],
    &["python", "-c"],
    &["python3", "-c"],
    &["mise", "help"],
    &["mise", "ls"],
    &["mise", "usage"],
    &[".venv/bin/ruff", "check"],
    &[".venv/bin/ruff", "format"],
    &["jar", "tf"],
    &["brew", "list"],
];

/// Approved while every argument after the prefix is a plain pattern; an option could change what the command does.
pub(crate) const APPROVED_PATTERN_PREFIXES: &[&[&str]] =
    &[&["git", "tag", "-l"], &["git", "tag", "--list"]];

pub(crate) const APPROVED_EXACT: &[&[&str]] = &[
    &["git", "config", "--list"],
    &["git", "remote"],
    &["git", "remote", "-v"],
    &["git", "remote", "--verbose"],
    &["git", "branch"],
    &["git", "branch", "-r"],
    &["git", "branch", "-a"],
    &["git", "branch", "--all"],
    &["git", "branch", "--show-current"],
    &["ssh", "-V"],
];

/// Directories whose commands are the same as their bare names (`/bin/rm` is `rm`).
const SYSTEM_BIN_DIRS: &[&str] = &[
    "/bin/",
    "/usr/bin/",
    "/usr/local/bin/",
    "/opt/homebrew/bin/",
    "/sbin/",
    "/usr/sbin/",
];

/// Approved when it is the only argument (`X --help`); anywhere else it says nothing about the rest.
pub(crate) const DERISKING_FLAGS: &[&str] = &["--help", "--version"];

/// Literals the command normalizer matches on directly.
pub(crate) const NORMALIZER_WORDS: &[&str] = &[
    "git",
    "debi",
    "mise",
    "exec",
    "x",
    "find",
    "sed",
    "source",
    ".",
    "set",
    ".venv/bin/activate",
    "--",
    ";",
    "+",
    "{}",
    "-delete",
    "-exec",
    "-execdir",
    "-ok",
    "-okdir",
];

pub(crate) const DANGEROUS_ENV_VARS: &[&str] = &[
    "PATH",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "BASH_ENV",
    "ENV",
    "NODE_OPTIONS",
    "PYTHONSTARTUP",
    "GIT_EXEC_PATH",
    "GIT_SSH_COMMAND",
    "GIT_TEMPLATE_DIR",
    "EDITOR",
    "VISUAL",
];

/// Programs the pager variables name; harmless as values, any other value runs an arbitrary program.
pub(crate) const PAGER_VARS: &[&str] = &["PAGER", "GIT_PAGER"];

/// Whether `NAME=value` in front of a command (or after `env`) could make an approved command run something else.
/// `value` is `None` when it is not a literal.
pub(crate) fn is_dangerous_assignment(name: &str, value: Option<&str>) -> bool {
    if PAGER_VARS.contains(&name) {
        return !value.is_some_and(is_plain_pager);
    }
    if name == "PATH" && value.is_some_and(is_system_path) {
        return false;
    }
    DANGEROUS_ENV_VARS.contains(&name) || name.starts_with("DYLD_")
}

/// `cat`, or `less` with any options.
fn is_plain_pager(command: &str) -> bool {
    let mut words = command.split_whitespace();
    match words.next() {
        Some("cat") => words.next().is_none(),
        Some("less") => words.all(|word| word.starts_with('-')),
        _ => false,
    }
}

/// A `PATH` made only of system directories, so every name still resolves to a system command.
fn is_system_path(value: &str) -> bool {
    value
        .split(':')
        .all(|dir| SYSTEM_BIN_DIRS.contains(&format!("{}/", dir.trim_end_matches('/')).as_str()))
}

pub(crate) fn system_command_name(name: &str) -> &str {
    SYSTEM_BIN_DIRS
        .iter()
        .find_map(|dir| name.strip_prefix(dir))
        .filter(|bare| !bare.is_empty() && !bare.contains('/'))
        .unwrap_or(name)
}

/// The name to look up in the command tables: `./.venv/bin/ruff` is `.venv/bin/ruff`, but `./ls` stays a local script.
pub(crate) fn table_name(name: &str) -> &str {
    name.strip_prefix("./")
        .filter(|rest| rest.contains('/'))
        .unwrap_or(name)
}

pub(crate) fn has_prefix(argv: &[&str], prefix: &[&str]) -> bool {
    argv.len() >= prefix.len() && argv[..prefix.len()] == *prefix
}

/// Words the rules (and the command normalizer) refer to; kept verbatim by redaction.
pub(crate) fn vocabulary() -> impl Iterator<Item = &'static str> {
    let prefixes = DENY_PREFIXES
        .iter()
        .map(|(prefix, _)| *prefix)
        .chain(KNOWN_PREFIXES.iter().map(|(prefix, _)| *prefix))
        .chain(USE_MISE_EXEMPT_PREFIXES.iter().copied())
        .chain(APPROVED_PREFIXES.iter().copied())
        .chain(APPROVED_PATTERN_PREFIXES.iter().copied())
        .chain(APPROVED_EXACT.iter().copied())
        .flatten()
        .copied();
    let singles = [
        SHELLS,
        WRAPPERS,
        USE_MISE_COMMANDS,
        APPROVED_COMMANDS,
        DERISKING_FLAGS,
        DANGEROUS_ENV_VARS,
        PAGER_VARS,
        NORMALIZER_WORDS,
    ]
    .into_iter()
    .flatten()
    .copied();
    prefixes.chain(singles)
}
