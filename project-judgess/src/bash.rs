//! Verdicts for Bash command lines: every simple command that would run is classified on its own (after the normalizer reduces wrappers and nested shells to the commands they run), and the worst outcome wins.

use crate::model::{Abstain, RuleId, Verdict};
use crate::rules::{self, has_prefix};
use crate::shell::{self, Arg, Command};

const MAX_DEPTH: usize = 16;

/// Deny > Defer > Unknown > Known > Approve, so the agent gets a redirect before anything else.
fn rank(verdict: &Verdict) -> u8 {
    match verdict {
        Verdict::Approve => 0,
        Verdict::Abstain(Abstain::Known { .. }) => 1,
        Verdict::Abstain(Abstain::Unknown { .. }) => 2,
        Verdict::Defer { .. } => 3,
        Verdict::Deny { .. } => 4,
    }
}

fn worst_of(verdicts: impl IntoIterator<Item = Verdict>) -> Verdict {
    verdicts.into_iter().fold(Verdict::Approve, |worst, next| {
        if rank(&next) > rank(&worst) {
            next
        } else {
            worst
        }
    })
}

#[derive(Clone, Copy)]
struct Context {
    /// Inside `mise exec --`, where the toolchain commands `use-mise` redirects to are exactly right.
    under_mise: bool,
    depth: usize,
}

impl Context {
    fn nested(self) -> Context {
        Context {
            depth: self.depth + 1,
            ..self
        }
    }
}

pub(crate) fn decide(source: &str) -> Verdict {
    script(
        source,
        Context {
            under_mise: false,
            depth: 0,
        },
    )
}

fn script(source: &str, ctx: Context) -> Verdict {
    if ctx.depth > MAX_DEPTH {
        return Verdict::unknown("nesting too deep");
    }
    let parsed = match shell::parse(source) {
        Ok(parsed) => parsed,
        Err(error) => return Verdict::unknown(format!("unparseable command: {error}")),
    };
    let opaque = parsed
        .opaque
        .into_iter()
        .map(|what| Verdict::unknown(format!("unsupported construct: {what}")));
    worst_of(opaque.chain(parsed.commands.iter().map(|command| simple(command, ctx))))
}

fn simple(command: &Command, ctx: Context) -> Verdict {
    if let Some(assignment) = command
        .assigned
        .iter()
        .find(|a| rules::is_dangerous_assignment(&a.name, a.value.literal()))
    {
        return dangerous_variable(&assignment.name);
    }
    classify(&command.argv(), ctx)
}

fn dangerous_variable(name: &str) -> Verdict {
    Verdict::unknown(format!("dangerous environment variable {name}"))
}

fn classify(argv: &[Arg], ctx: Context) -> Verdict {
    if ctx.depth > MAX_DEPTH {
        return Verdict::unknown("nesting too deep");
    }
    let Some(first) = argv.first() else {
        return Verdict::Approve;
    };
    let Some(raw_name) = first.literal() else {
        return Verdict::unknown("command name is not a literal");
    };
    let name = rules::table_name(rules::system_command_name(raw_name));
    let argv = [&[Arg::Literal(name.to_string())][..], &argv[1..]].concat();
    match name {
        _ if rules::WRAPPERS.contains(&name) => wrapper(name, &argv, ctx),
        _ if rules::SHELLS.contains(&name) => shell_invocation(&argv, ctx),
        "find" => find(&argv, ctx),
        "mise" => mise(&argv, ctx),
        _ => match by_table(&strip_git_options(&argv), ctx) {
            Verdict::Abstain(Abstain::Unknown { .. }) if raw_name.starts_with("./") => {
                Verdict::known(RuleId::LocalScript)
            }
            verdict => verdict,
        },
    }
}

/// Literal words of `argv`, with a placeholder for dynamic ones that never matches a rule.
fn words(argv: &[Arg]) -> Vec<&str> {
    argv.iter()
        .map(|arg| arg.literal().unwrap_or("\0"))
        .collect()
}

fn by_table(argv: &[Arg], ctx: Context) -> Verdict {
    let words = words(argv);
    if let Some(rule) = deny_rule(&words, ctx) {
        return Verdict::Deny { rule };
    }
    if let Some(rule) = known_rule(&words) {
        return Verdict::known(rule);
    }
    if is_approved(&words) || (ctx.under_mise && is_toolchain(&words)) {
        return Verdict::Approve;
    }
    Verdict::unknown(format!(
        "no rule for `{}`",
        words.iter().take(3).copied().collect::<Vec<_>>().join(" ")
    ))
}

fn deny_rule(words: &[&str], ctx: Context) -> Option<RuleId> {
    if let Some((_, rule)) = rules::DENY_PREFIXES
        .iter()
        .find(|(prefix, _)| has_prefix(words, prefix))
    {
        return Some(*rule);
    }
    (is_toolchain(words) && !ctx.under_mise).then_some(RuleId::UseMise)
}

fn is_toolchain(words: &[&str]) -> bool {
    rules::USE_MISE_COMMANDS.contains(&words[0])
        && !rules::USE_MISE_EXEMPT_PREFIXES
            .iter()
            .any(|prefix| has_prefix(words, prefix))
}

fn known_rule(words: &[&str]) -> Option<RuleId> {
    if let Some((_, rule)) = rules::KNOWN_PREFIXES
        .iter()
        .find(|(prefix, _)| has_prefix(words, prefix))
    {
        return Some(*rule);
    }
    if words[0] == "sed" && words[1..].iter().any(|arg| is_sed_in_place(arg)) {
        return Some(RuleId::IrreversibleSedInPlace);
    }
    None
}

fn is_sed_in_place(arg: &str) -> bool {
    match arg.strip_prefix("--") {
        Some(long) => long == "in-place" || long.starts_with("in-place="),
        None => arg.starts_with('-') && arg.contains(['i', 'I']),
    }
}

fn is_approved(words: &[&str]) -> bool {
    rules::APPROVED_COMMANDS.contains(&words[0])
        || rules::APPROVED_PREFIXES
            .iter()
            .any(|prefix| has_prefix(words, prefix))
        || rules::APPROVED_EXACT.contains(&words)
        || is_pattern_listing(words)
        || is_venv_activation(words)
        || is_set_options(words)
        || is_derisked(words)
}

/// `X --help` and `X --version` only show text, if `X` is a known program (a script by path may ignore its arguments).
/// Anywhere else the flag says nothing about the rest of the command (`bash -c 'rm x' --help` runs `rm`), except that `git <subcommand> --help` opens the manual instead of running.
fn is_derisked(words: &[&str]) -> bool {
    matches!(words, [name, flag] if !name.contains('/') && rules::DERISKING_FLAGS.contains(flag))
        || matches!(words, ["git", _, "--help"])
}

/// `set -e`, `set +e`, `set -euo pipefail`: shell options only, never positional parameters.
fn is_set_options(words: &[&str]) -> bool {
    let ["set", args @ ..] = words else {
        return false;
    };
    let mut args = args.iter().peekable();
    if args.peek().is_none() {
        return false;
    }
    while let Some(arg) = args.next() {
        let Some(flags) = arg
            .strip_prefix(['-', '+'])
            .filter(|flags| !flags.is_empty() && flags.chars().all(|c| c.is_ascii_alphabetic()))
        else {
            return false;
        };
        if flags.ends_with('o') {
            args.next_if(|name| !name.starts_with(['-', '+']));
        }
    }
    true
}

/// A listing command whose remaining arguments are patterns, never options that could turn it into a mutation.
fn is_pattern_listing(words: &[&str]) -> bool {
    rules::APPROVED_PATTERN_PREFIXES.iter().any(|prefix| {
        has_prefix(words, prefix)
            && words[prefix.len()..]
                .iter()
                .all(|arg| !arg.starts_with('-'))
    })
}

fn is_venv_activation(words: &[&str]) -> bool {
    matches!(words, ["source" | ".", path] if *path == ".venv/bin/activate" || path.ends_with("/.venv/bin/activate"))
}

/// Drops `-C <path>` and `--no-pager` ahead of the git subcommand; neither changes what the subcommand does.
fn strip_git_options(argv: &[Arg]) -> Vec<Arg> {
    let is_git = |arg: &Arg| arg.literal() == Some("git");
    let skip = usize::from(
        argv.first()
            .is_some_and(|arg| arg.literal() == Some("debi"))
            && argv.get(1).is_some_and(is_git),
    );
    if !argv.get(skip).is_some_and(is_git) {
        return argv.to_vec();
    }
    let mut stripped = argv[..=skip].to_vec();
    let mut rest = argv[skip + 1..].iter();
    let mut options_done = false;
    while let Some(arg) = rest.next() {
        match arg.literal() {
            Some("-C") if !options_done => {
                rest.next();
            }
            Some("--no-pager") if !options_done => {}
            _ => {
                options_done = true;
                stripped.push(arg.clone());
            }
        }
    }
    stripped
}

/// Arguments after leading options; `value_options` are the options that consume the following argument.
fn after_options<'a>(args: &'a [Arg], value_options: &[&str]) -> &'a [Arg] {
    let mut index = 0;
    while let Some(option) = args
        .get(index)
        .and_then(Arg::literal)
        .filter(|text| text.starts_with('-') && text.len() > 1)
    {
        index += if option == "--" {
            return &args[index + 1..];
        } else if value_options.contains(&option) {
            2
        } else {
            1
        };
    }
    args.get(index..).unwrap_or_default()
}

/// The command a wrapper runs, and the variables (name, value) it sets for it.
struct Unwrapped<'a> {
    inner: &'a [Arg],
    env: Vec<(&'a str, &'a str)>,
}

fn unwrap<'a>(name: &str, args: &'a [Arg]) -> Unwrapped<'a> {
    let plain = |inner| Unwrapped {
        inner,
        env: Vec::new(),
    };
    match name {
        "timeout" => plain(
            match after_options(args, &["-s", "--signal", "-k", "--kill-after"]) {
                [_duration, command @ ..] => command,
                [] => &[],
            },
        ),
        "nice" => plain(after_options(args, &["-n", "--adjustment"])),
        "xargs" => plain(after_options(
            args,
            &["-I", "-n", "-P", "-L", "-d", "-E", "-s", "-a", "-R", "-S"],
        )),
        "env" => {
            let mut inner = after_options(args, ENV_VALUE_OPTIONS);
            let mut env = Vec::new();
            while let Some(variable) = inner.first().and_then(Arg::literal).and_then(assignment) {
                env.push(variable);
                inner = &inner[1..];
            }
            Unwrapped { inner, env }
        }
        _ => plain(after_options(args, &[])),
    }
}

/// A script or command line that an option takes as its value.
enum Embedded {
    /// `-cVALUE`, `--long=VALUE`: the value is the tail of the word at this index of the arguments.
    Attached { word: usize, value: String },
    /// The value is the word at this index of the arguments.
    Next(usize),
}

/// Whether `word` is the option `-<short>` / `--<long>` that takes a script: `Some(None)` when the value is the next word, `Some(Some(value))` when it is attached (`-cVALUE`, `--long=VALUE`), including inside a short cluster.
fn embedded_option<'a>(word: &'a str, short: char, long: &str) -> Option<Option<&'a str>> {
    if let Some(name) = word.strip_prefix("--") {
        return if name == long {
            Some(None)
        } else {
            name.strip_prefix(long)?.strip_prefix('=').map(Some)
        };
    }
    let (_, tail) = word.strip_prefix('-')?.split_once(short)?;
    Some(Some(tail).filter(|tail| !tail.is_empty()))
}

/// `embedded_option` for the word at `index` of the arguments.
fn embedded_at(word: &str, index: usize, short: char, long: &str) -> Option<Embedded> {
    Some(match embedded_option(word, short, long)? {
        Some(value) => Embedded::Attached {
            word: index,
            value: value.to_string(),
        },
        None => Embedded::Next(index + 1),
    })
}

/// Runs the script that `embedded` locates in `args` as its own shell script.
fn embedded_script(args: &[Arg], embedded: Embedded, ctx: Context) -> Verdict {
    match embedded {
        Embedded::Attached { value, .. } => script(&value, ctx.nested()),
        Embedded::Next(index) => match args.get(index) {
            Some(Arg::Literal(source)) => script(source, ctx.nested()),
            _ => Verdict::unknown("script string is not a literal"),
        },
    }
}

const ENV_VALUE_OPTIONS: &[&str] = &["-u", "--unset", "-C", "--chdir", "-P"];

/// The `-S` / `--split-string` option of `env` among its leading options, and how many words it consumes.
fn env_split_string(args: &[Arg]) -> Option<(Embedded, usize)> {
    let mut index = 0;
    while let Some(option) = args
        .get(index)
        .and_then(Arg::literal)
        .filter(|text| text.starts_with('-') && text.len() > 1 && *text != "--")
    {
        match embedded_at(option, index, 'S', "split-string") {
            Some(embedded) => {
                let consumed = if matches!(embedded, Embedded::Next(_)) {
                    index + 2
                } else {
                    index + 1
                };
                return Some((embedded, consumed));
            }
            None => {
                index += if ENV_VALUE_OPTIONS.contains(&option) {
                    2
                } else {
                    1
                }
            }
        }
    }
    None
}

fn wrapper(name: &str, argv: &[Arg], ctx: Context) -> Verdict {
    let args = &argv[1..];
    if name == "command" && matches!(args.first().and_then(Arg::literal), Some("-v" | "-V")) {
        return Verdict::Approve;
    }
    if name == "watch" {
        return watch(args, ctx);
    }
    if name == "env"
        && let Some((string, consumed)) = env_split_string(args)
    {
        return if consumed < args.len() {
            Verdict::unknown("arguments after the `env -S` string")
        } else {
            embedded_script(args, string, ctx)
        };
    }
    let Unwrapped { inner, env } = unwrap(name, args);
    if let Some((variable, _)) = env
        .iter()
        .find(|(variable, value)| rules::is_dangerous_assignment(variable, Some(value)))
    {
        return dangerous_variable(variable);
    }
    match (inner.is_empty(), name) {
        (false, _) => classify(inner, ctx.nested()),
        (true, "xargs" | "env") => Verdict::Approve,
        (true, _) => Verdict::unknown(format!("`{name}` without a command")),
    }
}

/// `watch` joins its arguments and runs the result through `sh -c`.
fn watch(args: &[Arg], ctx: Context) -> Verdict {
    let command = after_options(args, &["-n", "--interval"]);
    let Some(words) = command.iter().map(Arg::literal).collect::<Option<Vec<_>>>() else {
        return Verdict::unknown("`watch` command is not a literal");
    };
    if words.is_empty() {
        return Verdict::unknown("`watch` without a command");
    }
    script(&words.join(" "), ctx.nested())
}

/// `NAME=value` as (name, value).
fn assignment(word: &str) -> Option<(&str, &str)> {
    let (name, value) = word.split_once('=')?;
    let mut chars = name.chars();
    let valid = chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    valid.then_some((name, value))
}

/// Where the script string of `bash -c <script>` sits in `args` (the arguments after the shell's name), if anywhere.
fn shell_script_index(args: &[Arg]) -> Option<usize> {
    let mut index = 0;
    let mut has_command_flag = false;
    while let Some(flag) = args.get(index).and_then(Arg::literal) {
        let is_short = flag.len() > 1 && (flag.starts_with('-') || flag.starts_with('+'));
        if !is_short || flag.starts_with("--") {
            break;
        }
        has_command_flag |= flag.starts_with('-') && flag.contains('c');
        // `-o pipefail`, `-eo pipefail`, `+O extglob`: the option name is the next word.
        index += if flag.ends_with('o') || flag.ends_with('O') {
            2
        } else {
            1
        };
    }
    has_command_flag.then_some(index)
}

fn shell_invocation(argv: &[Arg], ctx: Context) -> Verdict {
    let args = &argv[1..];
    if args.first().and_then(Arg::literal) == Some("-n") {
        return Verdict::Approve;
    }
    match shell_script_index(args) {
        None => Verdict::known(RuleId::ShellScript),
        Some(index) => embedded_script(args, Embedded::Next(index), ctx),
    }
}

/// The word of a command line that holds an embedded script, for redaction to rewrite.
pub(crate) struct ScriptWord {
    pub index: usize,
    /// For `-cSCRIPT` / `--command=SCRIPT`: how many trailing bytes of the word are the script.
    pub attached_len: Option<usize>,
}

/// Where in `argv` the script of a `bash -c` would be, looking through wrappers and `mise exec`.
pub(crate) fn script_word(argv: &[Arg]) -> Option<ScriptWord> {
    let locate = |embedded: &Embedded, args_start: usize| match embedded {
        Embedded::Next(index) => ScriptWord {
            index: args_start + index,
            attached_len: None,
        },
        Embedded::Attached { word, value } => ScriptWord {
            index: args_start + word,
            attached_len: Some(value.len()),
        },
    };
    let mut offset = 0;
    for _ in 0..MAX_DEPTH {
        let rest = &argv[offset..];
        let name = rules::table_name(rules::system_command_name(rest.first()?.literal()?));
        let args = &rest[1..];
        let args_start = argv.len() - args.len();
        let inner = match name {
            "env" => match env_split_string(args) {
                Some((embedded, _)) => return Some(locate(&embedded, args_start)),
                None => unwrap(name, args).inner,
            },
            "mise" => {
                let MiseCall::Exec(exec_args) = mise_call(args) else {
                    return None;
                };
                let (script, inner) = mise_exec_parts(exec_args);
                if let Some(embedded) = script {
                    return Some(locate(&embedded, argv.len() - exec_args.len()));
                }
                Some(inner).filter(|inner| !inner.is_empty())?
            }
            "watch" => return None,
            _ if rules::SHELLS.contains(&name) => {
                return shell_script_index(args).map(|index| ScriptWord {
                    index: args_start + index,
                    attached_len: None,
                });
            }
            _ if rules::WRAPPERS.contains(&name) => unwrap(name, args).inner,
            _ => return None,
        };
        offset = argv.len() - inner.len();
    }
    None
}

fn find(argv: &[Arg], ctx: Context) -> Verdict {
    let mut verdicts = Vec::new();
    let mut args = argv[1..].iter().peekable();
    while let Some(arg) = args.next() {
        match arg.literal() {
            Some("-delete") => verdicts.push(Verdict::known(RuleId::IrreversibleFindDelete)),
            Some("-exec" | "-execdir" | "-ok" | "-okdir") => {
                let mut command = Vec::new();
                for arg in args.by_ref() {
                    if matches!(arg.literal(), Some(";" | "+")) {
                        break;
                    }
                    command.push(arg.clone());
                }
                verdicts.push(if command.is_empty() {
                    Verdict::unknown("`find -exec` without a command")
                } else {
                    classify(&command, ctx.nested())
                });
            }
            _ => {}
        }
    }
    worst_of(verdicts)
}

const MISE_VALUE_OPTIONS: &[&str] = &[
    "-C",
    "--cd",
    "-E",
    "--env",
    "-j",
    "--jobs",
    "-P",
    "--profile",
];

const MISE_FLAG_OPTIONS: &[&str] = &[
    "-q",
    "--quiet",
    "-v",
    "-vv",
    "-vvv",
    "--verbose",
    "-y",
    "--yes",
    "--no-config",
    "--no-env",
    "--no-hooks",
    "--raw",
    "--locked",
    "--silent",
    "-h",
    "--help",
    "-V",
    "--version",
];

enum MiseCall<'a> {
    Exec(&'a [Arg]),
    Other,
    /// An option this code does not know precedes the subcommand, so it cannot tell whether a command is run.
    Unclear,
}

fn mise_call(args: &[Arg]) -> MiseCall<'_> {
    let mut index = 0;
    loop {
        match args.get(index) {
            None => return MiseCall::Other,
            Some(Arg::Dynamic) => return MiseCall::Unclear,
            Some(Arg::Literal(word)) if word.starts_with('-') => {
                index += if MISE_VALUE_OPTIONS.contains(&word.as_str()) {
                    2
                } else if MISE_FLAG_OPTIONS.contains(&word.as_str()) {
                    1
                } else {
                    return MiseCall::Unclear;
                };
            }
            Some(Arg::Literal(subcommand)) => {
                return if matches!(subcommand.as_str(), "exec" | "x") {
                    MiseCall::Exec(&args[index + 1..])
                } else {
                    MiseCall::Other
                };
            }
        }
    }
}

/// The `-c` / `--command` script of `mise exec`, and the command after `--`.
fn mise_exec_parts(exec_args: &[Arg]) -> (Option<Embedded>, &[Arg]) {
    let (options, inner) = match exec_args.iter().position(|arg| arg.literal() == Some("--")) {
        Some(separator) => (&exec_args[..separator], &exec_args[separator + 1..]),
        None => (exec_args, &[][..]),
    };
    let script = options
        .iter()
        .enumerate()
        .find_map(|(index, arg)| embedded_at(arg.literal()?, index, 'c', "command"));
    (script, inner)
}

fn mise(argv: &[Arg], ctx: Context) -> Verdict {
    let exec_args = match mise_call(&argv[1..]) {
        MiseCall::Other => return Verdict::Approve,
        MiseCall::Unclear => {
            return Verdict::unknown("cannot tell whether `mise` runs a command");
        }
        MiseCall::Exec(exec_args) => exec_args,
    };
    let ctx = Context {
        under_mise: true,
        ..ctx
    };
    let (script, inner) = mise_exec_parts(exec_args);
    let mut verdicts = Vec::new();
    if let Some(script) = script {
        verdicts.push(embedded_script(exec_args, script, ctx));
    }
    if !inner.is_empty() {
        verdicts.push(classify(inner, ctx.nested()));
    }
    if verdicts.is_empty() {
        return Verdict::unknown("`mise exec` without a command");
    }
    worst_of(verdicts)
}
