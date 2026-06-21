/**
 * Best-effort, cosmetic derivation of the directory name a pasted git URL would clone into.
 * Mirrors the Rust `parse_clone_target` (`src-tauri/src/repo_clone.rs`) for the common shapes, and is used ONLY for the "Clones to:" hint in {@link showCloneRepoDialog}.
 * The Rust parser is authoritative — when this returns `null` (input not yet a recognizable URL) the hint is simply hidden; the backend still validates.
 */
export function deriveCloneDirName(input: string): string | null {
  const trimmed = input.trim();
  if (trimmed === '') return null;

  // http(s): GitHub web URLs reduce to the repo (segment 1); other hosts use the last path segment.
  const httpMatch = /^https?:\/\/([^/]+)\/(.*)$/.exec(trimmed);
  if (httpMatch) {
    const host = httpMatch[1].toLowerCase();
    const segments = httpMatch[2]
      .split(/[?#]/)[0]
      .split('/')
      .filter((s) => s !== '');
    if (host === 'github.com' || host === 'www.github.com') {
      return segments.length >= 2 ? sanitize(segments[1]) : null;
    }
    return segments.length > 0 ? sanitize(segments[segments.length - 1]) : null;
  }

  // ssh:// / git:// / file:// — the last path segment.
  if (/^(?:ssh|git|file):\/\//.test(trimmed)) {
    return lastSegment(trimmed);
  }

  // scp-like git@host:owner/repo(.git)
  if (!trimmed.includes('://') && trimmed.includes('@') && trimmed.includes(':')) {
    return lastSegment(trimmed.slice(trimmed.indexOf(':') + 1));
  }

  return null;
}

function lastSegment(path: string): string | null {
  const last = path.split(/[?#]/)[0].replace(/\/+$/, '').split('/').pop() ?? '';
  return sanitize(last);
}

/** Strip a trailing `.git` and accept only a single safe path component. */
function sanitize(segment: string): string | null {
  const name = segment.endsWith('.git') ? segment.slice(0, -4) : segment;
  return name !== '' && name !== '..' && !name.includes('/') ? name : null;
}
