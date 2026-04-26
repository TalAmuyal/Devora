package wsgit

import "strings"

// PorcelainCounts is the per-category count derived from
// `git status --porcelain=v1 -z` output.
//
// Counting semantics:
//   - Untracked: lines starting with "??".
//   - Staged: any other entry whose index status (X) is not ' ' and not '?'.
//   - Unstaged: any other entry whose worktree status (Y) is not ' ' and not '?'.
//
// A file modified in both index and worktree (e.g. "MM") therefore counts in
// both Staged and Unstaged. Conflict states like "UU"/"DD" land in Unstaged
// because that's where the unresolved conflict lives; "AA" counts in both.
type PorcelainCounts struct {
	Staged    int
	Unstaged  int
	Untracked int
}

// parsePorcelain consumes `git status --porcelain=v1 -z` output (NUL-separated
// records) and returns aggregate PorcelainCounts. Renames emit two paths
// separated by NUL ("XY <to-path>\0<from-path>\0" — to-path first, from-path
// second); the trailing path is consumed but not counted again. Trailing bytes
// without a terminating NUL are discarded (every well-formed record from git
// ends with NUL).
func parsePorcelain(porcelain string) PorcelainCounts {
	var counts PorcelainCounts

	records := strings.Split(porcelain, "\x00")
	// The trailing element after the final NUL is always empty for well-
	// formed input. Drop everything from the last non-empty trailer onward
	// so partial records (no terminating NUL) are ignored.
	if len(records) > 0 {
		records = records[:len(records)-1]
	}

	i := 0
	for i < len(records) {
		entry := records[i]
		i++
		if len(entry) < 3 {
			continue
		}

		x := entry[0]
		y := entry[1]

		if x == '?' && y == '?' {
			counts.Untracked++
			continue
		}

		if isConflictPair(x, y) {
			counts.Unstaged++
			continue
		}

		if x != ' ' && x != '?' {
			counts.Staged++
		}
		if y != ' ' && y != '?' {
			counts.Unstaged++
		}

		// Renames/copies in v1 -z carry a second path entry: the to-path is
		// in the current record and the from-path follows in the next record
		// ("XY <to-path>\0<from-path>\0"). Skip the from-path so it isn't
		// counted as a separate change.
		if x == 'R' || x == 'C' {
			if i < len(records) {
				i++
			}
		}
	}
	return counts
}

// isConflictPair reports whether (x, y) form an unmerged-conflict status pair.
// Per git docs, conflicts are marked by 'U' on either side, plus the special
// pairs "AA" (both added) and "DD" (both deleted).
func isConflictPair(x, y byte) bool {
	if x == 'U' || y == 'U' {
		return true
	}
	if x == 'A' && y == 'A' {
		return true
	}
	if x == 'D' && y == 'D' {
		return true
	}
	return false
}
