package wsgit

import "testing"

func TestParsePorcelain(t *testing.T) {
	cases := []struct {
		name     string
		input    string
		expected PorcelainCounts
	}{
		{
			name:     "empty",
			input:    "",
			expected: PorcelainCounts{},
		},
		{
			name:     "staged only",
			input:    "M  foo\x00",
			expected: PorcelainCounts{Staged: 1},
		},
		{
			name:     "unstaged only",
			input:    " M foo\x00",
			expected: PorcelainCounts{Unstaged: 1},
		},
		{
			// MM means modified in both index and worktree, so it counts as
			// one Staged and one Unstaged change (each category counts the
			// file at most once).
			name:     "modified in both index and worktree",
			input:    "MM foo\x00",
			expected: PorcelainCounts{Staged: 1, Unstaged: 1},
		},
		{
			name:     "untracked",
			input:    "?? foo\x00",
			expected: PorcelainCounts{Untracked: 1},
		},
		{
			// In v1 -z format, a rename emits "XY <to-path>\0<from-path>\0"
			// — the to-path is the current record's path and is followed by
			// a NUL, then the from-path with its own trailing NUL.
			name:     "rename",
			input:    "R  new\x00old\x00",
			expected: PorcelainCounts{Staged: 1},
		},
		{
			// Conflict states (UU, AA, DD, etc.) are counted in Unstaged
			// because that's where the unresolved conflict lives.
			name:     "conflict UU",
			input:    "UU foo\x00",
			expected: PorcelainCounts{Unstaged: 1},
		},
		{
			// AA is "added by both" — an unmerged conflict. Counted as a
			// single Unstaged change.
			name:     "conflict AA",
			input:    "AA foo\x00",
			expected: PorcelainCounts{Unstaged: 1},
		},
		{
			name:     "conflict DD",
			input:    "DD foo\x00",
			expected: PorcelainCounts{Unstaged: 1},
		},
		{
			name:     "conflict AU",
			input:    "AU foo\x00",
			expected: PorcelainCounts{Unstaged: 1},
		},
		{
			name:     "multiple entries",
			input:    "M  staged.go\x00 M unstaged.go\x00?? untracked.go\x00MM both.go\x00",
			expected: PorcelainCounts{Staged: 2, Unstaged: 2, Untracked: 1},
		},
		{
			name:     "filename with spaces",
			input:    "M  has spaces in name.txt\x00",
			expected: PorcelainCounts{Staged: 1},
		},
		{
			// NUL-separated input handles filenames with newlines naturally.
			name:     "filename with newline",
			input:    "M  line1\nline2\x00",
			expected: PorcelainCounts{Staged: 1},
		},
		{
			name:     "deleted in worktree",
			input:    " D foo\x00",
			expected: PorcelainCounts{Unstaged: 1},
		},
		{
			name:     "added staged",
			input:    "A  foo\x00",
			expected: PorcelainCounts{Staged: 1},
		},
		{
			name:     "deleted staged",
			input:    "D  foo\x00",
			expected: PorcelainCounts{Staged: 1},
		},
		{
			name:     "rename mixed with other entries",
			input:    "R  new\x00old\x00M  other\x00?? extra\x00",
			expected: PorcelainCounts{Staged: 2, Untracked: 1},
		},
		{
			name:     "trailing data without NUL ignored",
			input:    "M  foo\x00trailing",
			expected: PorcelainCounts{Staged: 1},
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got := parsePorcelain(tc.input)
			if got != tc.expected {
				t.Fatalf("input %q: expected %+v, got %+v", tc.input, tc.expected, got)
			}
		})
	}
}
