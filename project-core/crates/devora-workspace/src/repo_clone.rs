//! Parsing a user-pasted git remote into the URL to hand `git clone` plus the destination folder name.
//!
//! Accepts the shapes a user is likely to copy from GitHub — a web/HTTPS URL (including repo-page URLs like `…/repo/pulls` or `…/repo#readme`), an SSH remote (`git@github.com:org/repo.git`) — as well as any other clonable URL (`ssh://`, `git://`, `file://`, GitHub Enterprise HTTPS).
//! GitHub web URLs are normalized down to the repository root (page URLs are not clonable); every other form is passed through unchanged so the protocol the user pasted is preserved.

use std::path::{Component, Path};

/// A parsed clone request: the URL to pass to `git clone` and the directory name to create under `repos/`.
pub struct CloneTarget {
    pub clone_url: String,
    pub dir_name: String,
}

/// Normalize `input` into a [`CloneTarget`], or `Err` with a user-facing message when it is unparseable.
pub fn parse_clone_target(input: &str) -> Result<CloneTarget, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Enter a repository URL".to_string());
    }

    // GitHub web / HTTPS URLs: normalize to the repo root (a page URL like `…/repo/pulls` is not clonable).
    if let Some(rest) = strip_http_prefix(trimmed) {
        return parse_http(trimmed, rest);
    }

    // ssh:// / git:// / file:// — pass the URL through, deriving the dir from the last path segment.
    if trimmed.starts_with("ssh://") || trimmed.starts_with("git://") || trimmed.starts_with("file://")
    {
        return Ok(CloneTarget {
            clone_url: trimmed.to_string(),
            dir_name: dir_from_path(trimmed)?,
        });
    }

    // SSH scp-like form: `git@host:owner/repo(.git)` (no scheme, has user@host and a `:` path separator).
    if !trimmed.contains("://") && trimmed.contains('@') {
        if let Some((_, path)) = trimmed.split_once(':') {
            return Ok(CloneTarget {
                clone_url: trimmed.to_string(),
                dir_name: dir_from_path(path)?,
            });
        }
    }

    Err(format!("Unrecognized repository URL: {trimmed}"))
}

fn strip_http_prefix(s: &str) -> Option<&str> {
    s.strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
}

/// Parse the part of an http(s) URL after the scheme: `host/owner/repo…`.
fn parse_http(original: &str, rest: &str) -> Result<CloneTarget, String> {
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    // Drop query/fragment, then split into non-empty path segments.
    let path = path.split(['?', '#']).next().unwrap_or("");
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if host.eq_ignore_ascii_case("github.com") || host.eq_ignore_ascii_case("www.github.com") {
        let (Some(owner), Some(repo)) = (segments.first(), segments.get(1)) else {
            return Err(format!("Not a GitHub repository URL: {original}"));
        };
        let repo = strip_git_suffix(repo);
        return Ok(CloneTarget {
            clone_url: format!("https://github.com/{owner}/{repo}"),
            dir_name: validate_dir_name(repo)?,
        });
    }

    // Generic https git URL (e.g. GitHub Enterprise): pass through, derive the dir from the last segment.
    let last = segments.last().copied().unwrap_or("");
    Ok(CloneTarget {
        clone_url: original.to_string(),
        dir_name: validate_dir_name(strip_git_suffix(last))?,
    })
}

/// Derive a directory name from a URL/path's last segment (query/fragment and trailing slash stripped, `.git` removed).
fn dir_from_path(path: &str) -> Result<String, String> {
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let last = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    validate_dir_name(strip_git_suffix(last))
}

fn strip_git_suffix(s: &str) -> &str {
    s.strip_suffix(".git").unwrap_or(s)
}

/// Ensure the derived name is a single, safe path component (no empty / `..` / separators).
fn validate_dir_name(name: &str) -> Result<String, String> {
    let mut components = Path::new(name).components();
    let single_normal =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    if name.is_empty() || !single_normal {
        return Err(format!("Could not derive a repository name from: {name}"));
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> CloneTarget {
        parse_clone_target(input).unwrap_or_else(|e| panic!("parse({input:?}) failed: {e}"))
    }

    #[test]
    fn https_with_git_suffix() {
        let t = parse("https://github.com/org/repo.git");
        assert_eq!(t.clone_url, "https://github.com/org/repo");
        assert_eq!(t.dir_name, "repo");
    }

    #[test]
    fn https_without_suffix() {
        let t = parse("https://github.com/org/repo");
        assert_eq!(t.clone_url, "https://github.com/org/repo");
        assert_eq!(t.dir_name, "repo");
    }

    #[test]
    fn https_trailing_slash() {
        let t = parse("https://github.com/org/repo/");
        assert_eq!(t.clone_url, "https://github.com/org/repo");
        assert_eq!(t.dir_name, "repo");
    }

    #[test]
    fn https_page_url_pulls() {
        let t = parse("https://github.com/org/repo/pulls");
        assert_eq!(t.clone_url, "https://github.com/org/repo");
        assert_eq!(t.dir_name, "repo");
    }

    #[test]
    fn https_with_fragment() {
        let t = parse("https://github.com/org/repo#");
        assert_eq!(t.clone_url, "https://github.com/org/repo");
        assert_eq!(t.dir_name, "repo");

        let t = parse("https://github.com/org/repo#readme");
        assert_eq!(t.clone_url, "https://github.com/org/repo");
    }

    #[test]
    fn https_with_query() {
        let t = parse("https://github.com/org/repo?tab=readme");
        assert_eq!(t.clone_url, "https://github.com/org/repo");
        assert_eq!(t.dir_name, "repo");
    }

    #[test]
    fn https_www_host() {
        let t = parse("https://www.github.com/org/repo");
        assert_eq!(t.clone_url, "https://github.com/org/repo");
        assert_eq!(t.dir_name, "repo");
    }

    #[test]
    fn ssh_scp_like_with_suffix() {
        let t = parse("git@github.com:org/repo.git");
        assert_eq!(t.clone_url, "git@github.com:org/repo.git");
        assert_eq!(t.dir_name, "repo");
    }

    #[test]
    fn ssh_scp_like_without_suffix() {
        let t = parse("git@github.com:org/repo");
        assert_eq!(t.clone_url, "git@github.com:org/repo");
        assert_eq!(t.dir_name, "repo");
    }

    #[test]
    fn ssh_url_form() {
        let t = parse("ssh://git@github.com/org/repo.git");
        assert_eq!(t.clone_url, "ssh://git@github.com/org/repo.git");
        assert_eq!(t.dir_name, "repo");
    }

    #[test]
    fn file_url() {
        let t = parse("file:///var/tmp/fixtures/repo.git");
        assert_eq!(t.clone_url, "file:///var/tmp/fixtures/repo.git");
        assert_eq!(t.dir_name, "repo");
    }

    #[test]
    fn github_enterprise_https_passthrough() {
        let t = parse("https://ghe.corp.example/team/repo.git");
        assert_eq!(t.clone_url, "https://ghe.corp.example/team/repo.git");
        assert_eq!(t.dir_name, "repo");
    }

    #[test]
    fn rejects_empty_and_garbage() {
        for input in ["", "   ", "not a url", "org/repo", "github.com/org/repo"] {
            assert!(
                parse_clone_target(input).is_err(),
                "expected {input:?} to be rejected"
            );
        }
    }

    #[test]
    fn rejects_github_url_without_repo() {
        assert!(parse_clone_target("https://github.com/org").is_err());
        assert!(parse_clone_target("https://github.com/").is_err());
    }
}
