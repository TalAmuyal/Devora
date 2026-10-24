//! Turns a raw request into a shareable one.
//! Nothing survives unless it is vocabulary (what the rules and common tooling talk about); everything else becomes a keyed hash, so cases keep their shape but not their content.
//!
//! Commands are rewritten in place using the parser's word spans, so the output is the original command text with only the private words replaced.

use std::collections::HashSet;
use std::ops::Range;

use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::Sha256;

use crate::bash::script_word;
use crate::cases::{CaseInput, REDACTED_HOME, decision_input};
use crate::rules;
use crate::shell::{self, Arg, Part, WordInfo};

const COMMANDS: &[&str] = &[
    "ls",
    "cd",
    "cat",
    "echo",
    "grep",
    "rg",
    "find",
    "sed",
    "awk",
    "sort",
    "uniq",
    "head",
    "tail",
    "wc",
    "cp",
    "mv",
    "rm",
    "mkdir",
    "rmdir",
    "touch",
    "chmod",
    "chown",
    "ln",
    "diff",
    "patch",
    "tar",
    "zip",
    "unzip",
    "gzip",
    "curl",
    "wget",
    "ssh",
    "scp",
    "rsync",
    "git",
    "gh",
    "jq",
    "yq",
    "xargs",
    "tee",
    "tr",
    "cut",
    "paste",
    "date",
    "sleep",
    "pwd",
    "env",
    "export",
    "unset",
    "source",
    "set",
    "read",
    "printf",
    "test",
    "true",
    "false",
    "kill",
    "ps",
    "top",
    "lsof",
    "which",
    "type",
    "file",
    "stat",
    "du",
    "df",
    "ping",
    "make",
    "cmake",
    "gcc",
    "clang",
    "rustc",
    "rustup",
    "pip",
    "pip3",
    "uv",
    "pytest",
    "mypy",
    "pnpm",
    "yarn",
    "deno",
    "java",
    "javac",
    "gradle",
    "mvn",
    "docker",
    "kubectl",
    "terraform",
    "brew",
    "nvim",
    "vim",
    "less",
    "more",
    "crit",
    "claude",
    "debi",
    "ccc",
    "tauri",
    "codesign",
    "xcodebuild",
    "swift",
    "python3.12",
    "python3.13",
    "pytest",
    "default",
    "run",
    "build",
    "test",
    "install",
    "check",
    "lint",
    "format",
    "fmt",
    "status",
    "diff",
    "log",
    "show",
    "branch",
    "commit",
    "push",
    "pull",
    "merge",
    "rebase",
    "stash",
    "remote",
    "tag",
    "clone",
    "init",
    "config",
    "list",
    "version",
    "help",
    "add",
    "remove",
    "update",
    "clean",
    "start",
    "stop",
    "dev",
    "main",
    "master",
    "origin",
    "HEAD",
    "HOME",
    "USER",
    "PWD",
    "SHELL",
    "LANG",
    "TERM",
    "TMPDIR",
    "MISE_VERBOSE",
    "pipefail",
];

/// Single-dash options longer than a cluster of flag letters (`find` primaries, `java -version`, ...).
const LONG_SINGLE_DASH_OPTIONS: &[&str] = &[
    "-name",
    "-iname",
    "-type",
    "-path",
    "-ipath",
    "-not",
    "-maxdepth",
    "-mindepth",
    "-mtime",
    "-newer",
    "-size",
    "-empty",
    "-prune",
    "-print",
    "-print0",
    "-regex",
    "-perm",
    "-version",
    "-classpath",
    "-cp",
];

const PATH_SEGMENTS: &[&str] = &[
    "/",
    ".",
    "..",
    "~",
    "tmp",
    "var",
    "usr",
    "bin",
    "sbin",
    "lib",
    "etc",
    "dev",
    "null",
    "opt",
    "home",
    "local",
    "homebrew",
    "share",
    "Users",
    "user",
    "Library",
    "Applications",
    "private",
    "Volumes",
    "Documents",
    "Downloads",
    "Desktop",
    ".git",
    ".venv",
    "node_modules",
    "src",
    "tests",
    "test",
    "scripts",
    "docs",
    "tools",
    "target",
    "dist",
    "build",
    "out",
    ".claude",
    ".crit",
    ".config",
    ".cache",
    ".local",
    ".cargo",
    ".npm",
    ".github",
    "plans",
    "reviews",
    "debug",
    "release",
    "__pycache__",
    "site-packages",
    "vendor",
    "pkg",
    "cmd",
    "internal",
    "examples",
    "include",
    "worktrees",
    "README.md",
    "CLAUDE.md",
    "CHANGELOG.md",
    "LICENSE",
    "Makefile",
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "package-lock.json",
    "go.mod",
    "go.sum",
    "mise.toml",
    "pyproject.toml",
    "tsconfig.json",
    ".gitignore",
    ".bak",
    "stdin",
    "stdout",
    "stderr",
    "fd",
    "activate",
];

const EXTENSIONS: &[&str] = &[
    "rs", "py", "go", "ts", "tsx", "js", "jsx", "mjs", "json", "toml", "yaml", "yml", "md", "txt",
    "sh", "zsh", "bash", "html", "css", "lock", "log", "c", "h", "cpp", "hpp", "java", "kt",
    "swift", "rb", "lua", "vim", "sql", "xml", "csv", "png", "jpg", "svg", "pdf", "zip", "tar",
    "gz", "tgz", "dmg", "app", "plist", "jar", "so", "dylib", "a", "o",
];

const HOSTS: &[&str] = &[
    "github.com",
    "api.github.com",
    "raw.githubusercontent.com",
    "gist.github.com",
    "docs.anthropic.com",
    "code.claude.com",
    "claude.ai",
    "anthropic.com",
    "example.com",
    "localhost",
    "127.0.0.1",
    "crates.io",
    "docs.rs",
    "pypi.org",
    "npmjs.com",
    "www.npmjs.com",
    "go.dev",
    "pkg.go.dev",
    "developer.mozilla.org",
    "stackoverflow.com",
];

pub(crate) struct Redactor {
    key: Vec<u8>,
    home: Option<String>,
    user: Option<String>,
    vocabulary: HashSet<&'static str>,
}

impl Redactor {
    pub(crate) fn new(key: Vec<u8>, home: Option<String>) -> Redactor {
        let user = home
            .as_deref()
            .and_then(|home| home.rsplit('/').next())
            .filter(|name| !name.is_empty())
            .map(str::to_string);
        let vocabulary = rules::vocabulary()
            .chain(COMMANDS.iter().copied())
            .chain(PATH_SEGMENTS.iter().copied())
            .chain(LONG_SINGLE_DASH_OPTIONS.iter().copied())
            .collect();
        Redactor {
            key,
            home,
            user,
            vocabulary,
        }
    }

    pub(crate) fn input(
        &self,
        tool_name: &str,
        cwd: Option<&str>,
        tool_input: &Value,
    ) -> CaseInput {
        CaseInput {
            tool_name: tool_name.to_string(),
            cwd: cwd.map(|cwd| self.path(cwd)),
            tool_input: self.value(
                tool_name == "Bash",
                None,
                &decision_input(tool_name, tool_input),
            ),
        }
    }

    /// Whether the text still carries this machine's home directory or username.
    pub(crate) fn leaks(&self, text: &str) -> bool {
        let home = self.home.as_deref().filter(|home| home.len() > 1);
        let user = self.user.as_deref().filter(|user| user.len() >= 3);
        home.is_some_and(|home| contains_token(text, home))
            || user.is_some_and(|user| contains_token(text, user))
    }

    fn value(&self, is_bash: bool, field: Option<&str>, value: &Value) -> Value {
        match value {
            Value::String(text) => Value::String(match field {
                Some("command") if is_bash => self.shell(text),
                Some("file_path" | "path" | "notebook_path") => self.path(text),
                Some("url") => self.url(text),
                _ => self.word(text),
            }),
            Value::Array(items) => Value::Array(
                items
                    .iter()
                    .map(|item| self.value(is_bash, field, item))
                    .collect(),
            ),
            Value::Object(fields) => Value::Object(
                fields
                    .iter()
                    .map(|(name, item)| (name.clone(), self.value(is_bash, Some(name), item)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }

    fn hash(&self, text: &str) -> String {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&self.key).expect("HMAC accepts keys of any length");
        mac.update(text.as_bytes());
        mac.finalize()
            .into_bytes()
            .iter()
            .take(3)
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()[..5]
            .to_string()
    }

    fn text(&self, text: &str) -> String {
        format!("text-{}", self.hash(text))
    }

    fn shell(&self, source: &str) -> String {
        let parsed = match shell::parse(source) {
            Ok(parsed) if !parsed.unspanned && parsed.opaque.is_empty() => parsed,
            _ => return self.text(source),
        };
        let mut replacements: Vec<(Range<usize>, String)> = Vec::new();
        for command in &parsed.commands {
            let script = script_word(&command.argv());
            for (index, word) in command.words.iter().enumerate() {
                match (script.as_ref().filter(|s| s.index == index), word.arg()) {
                    (Some(script), Arg::Literal(text)) => {
                        let (option, inner) =
                            text.split_at(text.len() - script.attached_len.unwrap_or(text.len()));
                        let redacted = format!("{option}{}", single_quoted(&self.shell(inner)));
                        replacements.push((word.span.clone(), redacted));
                    }
                    _ => self.word_replacements(source, word, &mut replacements),
                }
            }
        }
        for word in &parsed.other_words {
            self.word_replacements(source, word, &mut replacements);
        }
        apply(source, replacements).unwrap_or_else(|| self.text(source))
    }

    fn push_replacement(
        &self,
        replacements: &mut Vec<(Range<usize>, String)>,
        span: Range<usize>,
        original: &str,
        new: String,
    ) {
        if new != original {
            replacements.push((span, new));
        }
    }

    fn word_replacements(
        &self,
        source: &str,
        word: &WordInfo,
        replacements: &mut Vec<(Range<usize>, String)>,
    ) {
        if let shell::Arg::Literal(text) = word.arg() {
            if !text.is_empty() {
                self.push_replacement(replacements, word.span.clone(), &text, self.fragment(&text));
            }
            return;
        }
        for part in &word.parts {
            match part {
                Part::Literal { span, text } if !span.is_empty() => {
                    self.push_replacement(replacements, span.clone(), text, self.fragment(text));
                }
                Part::Variable { span, name } => {
                    let raw = &source[span.clone()];
                    let kept = match name {
                        Some(name) => self.is_known(name),
                        None => is_special_parameter(raw),
                    };
                    if !kept {
                        replacements.push((span.clone(), format!("${{var_{}}}", self.hash(raw))));
                    }
                }
                _ => {}
            }
        }
    }

    fn variable_name(&self, name: &str) -> String {
        if self.is_known(name) {
            name.to_string()
        } else {
            format!("var_{}", self.hash(name))
        }
    }

    /// Like `word`, but leaves surrounding whitespace alone (it separates heredoc lines from their terminator).
    fn fragment(&self, text: &str) -> String {
        let core = text.trim();
        if core.is_empty() {
            return text.to_string();
        }
        let start = text.len() - text.trim_start().len();
        format!(
            "{}{}{}",
            &text[..start],
            self.word(core),
            &text[start + core.len()..]
        )
    }

    pub(crate) fn word(&self, text: &str) -> String {
        if text.is_empty() || is_number(text) || self.is_known(text) {
            return text.to_string();
        }
        let plain = !text.contains(char::is_whitespace);
        if let Some(flag) = text.strip_prefix('-').filter(|_| plain) {
            return match flag.split_once('=') {
                Some((name, value)) if is_flag_name(name) => {
                    format!("-{}={}", self.flag(name), self.word(value))
                }
                _ if is_flag_name(flag) => format!("-{}", self.flag(flag)),
                // An option with its value attached (`-i.bak`, `-I{}`): the option stays, the value is judged alone.
                _ if flag.starts_with(|c: char| c.is_ascii_alphanumeric()) => {
                    format!("-{}{}", &flag[..1], self.word(&flag[1..]))
                }
                _ => self.text(text),
            };
        }
        if plain && text.contains("://") {
            return self.url(text);
        }
        if let Some((name, value)) = text
            .split_once('=')
            .filter(|(name, _)| plain && is_identifier(name))
        {
            return format!("{}={}", self.variable_name(name), self.word(value));
        }
        let has_extension = text
            .rsplit_once('.')
            .is_some_and(|(stem, ext)| !stem.is_empty() && EXTENSIONS.contains(&ext));
        if plain && (text.contains('/') || text.starts_with('~') || has_extension) {
            return self.path(text);
        }
        self.text(text)
    }

    /// The name of an option (without its leading dash).
    /// A long option stays; a short one keeps its first letter, and whatever follows survives only if it is vocabulary or a short cluster of flag letters (`-la`), since anything longer is probably a value attached to the option (`-uroot`, `-mCommit message`).
    fn flag(&self, name: &str) -> String {
        if name.starts_with('-')
            || name.len() <= MAX_FLAG_CLUSTER
            || self.is_known(&format!("-{name}"))
        {
            return name.to_string();
        }
        let first = name.chars().next().map_or(0, char::len_utf8);
        format!("{}{}", &name[..first], self.word(&name[first..]))
    }

    fn is_known(&self, text: &str) -> bool {
        self.vocabulary.contains(text) || is_redacted_token(text)
    }

    fn path(&self, text: &str) -> String {
        let (prefix, rest) = self.split_home(text);
        let segments: Vec<String> = rest
            .split('/')
            .map(|segment| self.segment(segment))
            .collect();
        format!("{prefix}{}", segments.join("/"))
    }

    fn split_home<'a>(&self, text: &'a str) -> (&'static str, &'a str) {
        for home in self.home.iter().map(String::as_str).chain([REDACTED_HOME]) {
            if let Some(rest) = text
                .strip_prefix(home)
                .filter(|rest| rest.is_empty() || rest.starts_with('/'))
            {
                return (REDACTED_HOME, rest);
            }
        }
        ("", text)
    }

    fn segment(&self, segment: &str) -> String {
        if segment.is_empty() || self.is_known(segment) {
            return segment.to_string();
        }
        if self.user.as_deref() == Some(segment) {
            return "user".to_string();
        }
        match segment.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() && EXTENSIONS.contains(&ext) => {
                format!("{}.{ext}", self.stem(stem))
            }
            _ => self.stem(segment),
        }
    }

    fn stem(&self, stem: &str) -> String {
        // Glob patterns are structure, not content.
        if stem.chars().all(|c| "*?[]{},.".contains(c)) || self.is_known(stem) {
            stem.to_string()
        } else {
            format!("seg-{}", self.hash(stem))
        }
    }

    fn url(&self, text: &str) -> String {
        let Some((scheme, rest)) = text
            .split_once("://")
            .filter(|(scheme, _)| scheme.chars().all(|c| c.is_ascii_alphanumeric()))
        else {
            return self.text(text);
        };
        let (location, query) = match rest.find(['?', '#']) {
            Some(index) => (&rest[..index], &rest[index..]),
            None => (rest, ""),
        };
        let (host, path) = location
            .split_once('/')
            .map_or((location, ""), |(host, path)| {
                (host, &location[host.len()..][..path.len() + 1])
            });
        let host = if HOSTS.contains(&host) || host.is_empty() || is_redacted_token(host) {
            host.to_string()
        } else {
            format!("host-{}", self.hash(host))
        };
        let query = if query.is_empty() {
            String::new()
        } else {
            format!("{}{}", &query[..1], self.text(&query[1..]))
        };
        format!(
            "{scheme}://{host}{}{query}",
            if path.is_empty() {
                String::new()
            } else {
                self.path(path)
            }
        )
    }
}

/// Applies non-overlapping replacements; `None` if they overlap or fall inside a character.
fn apply(source: &str, mut replacements: Vec<(Range<usize>, String)>) -> Option<String> {
    replacements.sort_by_key(|(span, _)| span.start);
    let mut out = String::with_capacity(source.len());
    let mut cursor = 0;
    for (span, new) in replacements {
        if span.start < cursor
            || !source.is_char_boundary(span.start)
            || !source.is_char_boundary(span.end)
        {
            return None;
        }
        out.push_str(&source[cursor..span.start]);
        out.push_str(&new);
        cursor = span.end;
    }
    out.push_str(&source[cursor..]);
    Some(out)
}

fn single_quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// Option clusters up to this many letters after the dash are flags (`-rf`, `-lah`); longer ones carry a value.
const MAX_FLAG_CLUSTER: usize = 3;

/// Numbers of up to this many digits (counts, ports, durations) are not private; account ids and phone numbers are.
const MAX_PUBLIC_DIGITS: usize = 4;

/// A small number: `20`, `1.5`, `30s`. Dotted quads like IP addresses and long digit runs are not.
fn is_number(text: &str) -> bool {
    let digits = text.strip_suffix(['s', 'm', 'h', 'd']).unwrap_or(text);
    let mut parts = digits.split('.');
    let small = |part: Option<&str>| {
        part.is_some_and(|part| {
            !part.is_empty()
                && part.len() <= MAX_PUBLIC_DIGITS
                && part.chars().all(|c| c.is_ascii_digit())
        })
    };
    small(parts.next())
        && parts.next().is_none_or(|part| small(Some(part)))
        && parts.next().is_none()
}

/// Whether `needle` occurs in `text` as a whole token, not inside a longer word (`tal` is not in `install`).
fn contains_token(text: &str, needle: &str) -> bool {
    text.match_indices(needle).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        let after = text[start + needle.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

fn is_flag_name(text: &str) -> bool {
    let name = text.strip_prefix('-').unwrap_or(text);
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_special_parameter(raw: &str) -> bool {
    let name = raw.trim_start_matches('$').trim_matches(['{', '}']);
    name.len() == 1 && "@*#?$!-0123456789".contains(name)
        || name.chars().all(|c| c.is_ascii_digit())
}

/// Output of an earlier redaction, so redacting twice changes nothing.
fn is_redacted_token(text: &str) -> bool {
    let stem = text.split('.').next().unwrap_or(text);
    ["seg-", "text-", "host-", "var_"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|hash| {
            hash.len() == 5
                && hash
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn redactor() -> Redactor {
        Redactor::new(b"test-key".to_vec(), Some("/Users/jane".into()))
    }

    #[test]
    fn private_content_is_replaced_and_structure_kept() {
        let redactor = redactor();
        let input = serde_json::json!({
            "command": "cd /Users/jane/secret-project && git commit -m \"Fix jane's payroll bug\" && bash -c 'ls /Users/jane/x'",
            "description": "Commit the payroll fix",
        });
        let redacted = redactor.input("Bash", Some("/Users/jane/secret-project"), &input);
        let text = serde_json::to_string(&redacted).unwrap();

        for private in ["jane", "secret-project", "payroll", "Commit the"] {
            assert!(!text.contains(private), "{private} leaked: {text}");
        }
        let command = redacted.tool_input["command"].as_str().unwrap();
        assert!(command.starts_with("cd /Users/user/seg-"), "{command}");
        assert!(command.contains(" && git commit -m "), "{command}");
        assert!(command.contains("bash -c '"), "{command}");

        let again = redactor.input("Bash", redacted.cwd.as_deref(), &redacted.tool_input);
        assert_eq!(again, redacted);
    }

    #[test]
    fn the_same_private_name_hashes_the_same_everywhere() {
        let redacted = redactor().shell("cd /Users/jane/proj && git -C /Users/jane/proj status");
        let hashes: Vec<&str> = redacted.matches("seg-").collect();
        assert_eq!(hashes.len(), 2);
        let first = redacted.split("seg-").nth(1).unwrap()[..5].to_string();
        let second = redacted.split("seg-").nth(2).unwrap()[..5].to_string();
        assert_eq!(first, second);
    }

    #[test]
    fn private_values_do_not_survive_in_any_position() {
        let redactor = Redactor::new(b"test-key".to_vec(), Some("/Users/tal".into()));
        let cases = [
            (
                "mysql -uroot -pS3cretPass acmedb",
                &["root", "S3cretPass", "acmedb"][..],
            ),
            ("git commit -mwip-acme-payroll", &["acme", "payroll"]),
            ("ssh 10.20.30.40 uptime", &["10.20.30.40"]),
            (
                "aws sts get-caller-identity --account 123456789012",
                &["123456789012"],
            ),
            (
                "curl https://tal:hunter2@github.com/acme/private-repo.git",
                &["hunter2", "acme", "private-repo"],
            ),
            (
                "curl -H 'Authorization: Bearer ghp_abcdef123' 'https://api.acme-corp.com/v1/users?token=xyz'",
                &["ghp_abcdef123", "acme", "xyz"],
            ),
            ("GITHUB_TOKEN=ghp_abc gh api user", &["ghp_abc"]),
            (
                "git log --author=renkinjitsu@gmail.com",
                &["renkinjitsu", "gmail"],
            ),
            ("ls ~tal/acme", &["tal", "acme"]),
            ("cat /Users/talia/notes", &["talia"]),
        ];
        for (command, private) in cases {
            let redacted = redactor.shell(command);
            for secret in private {
                assert!(
                    !redacted.contains(secret),
                    "{secret} leaked from `{command}`: {redacted}"
                );
            }
        }
        for kept in [
            "ls -la",
            "rm -rf x",
            "sed -En -I",
            "head -n 20",
            "timeout 30 ls",
            "mise -j 4 exec -- ls",
        ] {
            assert_eq!(redactor.shell(kept), kept);
        }
    }

    #[test]
    fn the_username_only_leaks_as_a_whole_token() {
        let redactor = Redactor::new(b"test-key".to_vec(), Some("/Users/tal".into()));
        assert!(!redactor.leaks(r#"{"command":"cargo install ripgrep"}"#));
        assert!(!redactor.leaks(r#"{"command":"cat /Users/user/talia"}"#));
        assert!(redactor.leaks(r#"{"command":"cat /Users/tal/x"}"#));
        assert!(redactor.leaks(r#"{"command":"echo tal"}"#));
    }
}
