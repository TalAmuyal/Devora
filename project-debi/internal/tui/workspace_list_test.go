package tui

import (
	"strings"
	"testing"

	"charm.land/lipgloss/v2"

	"devora/internal/style"
)

func findRepoRow(t *testing.T, rendered string) string {
	t.Helper()
	for _, line := range strings.Split(rendered, "\n") {
		if strings.Contains(line, "\u2713") || strings.Contains(line, "\u2717") {
			return line
		}
	}
	t.Fatalf("no repo row found in rendered output:\n%s", rendered)
	return ""
}

func TestRenderCard_RepoRow(t *testing.T) {
	styles := NewStyles(style.ThemePalette{})
	m := NewWorkspaceListModel(&styles)

	tests := []struct {
		name           string
		innerWidth     int
		info           WorkspaceInfo
		mustContain    []string
		mustNotContain []string
	}{
		{
			name:       "short values fit without truncation",
			innerWidth: 84,
			info: WorkspaceInfo{
				Name:     "ws",
				Category: CategoryActiveNoSession,
				RepoGitStatuses: map[string]RepoGitStatus{
					"tals-setup": {Branch: "HEAD", IsClean: true},
				},
			},
			mustContain:    []string{"tals-setup", "HEAD", "\u2713 clean"},
			mustNotContain: []string{"\u2026"},
		},
		{
			name:       "long branch is tail-truncated on a dirty repo",
			innerWidth: 84,
			info: WorkspaceInfo{
				Name:     "ws",
				Category: CategoryActiveNoSession,
				RepoGitStatuses: map[string]RepoGitStatus{
					"repo-a": {
						Branch:  "tal-amuyal-fix-merchant-ax-prev-minute-request-count-returning-zeros",
						IsClean: false,
					},
				},
			},
			mustContain:    []string{"\u2717 dirty", "\u2026"},
			mustNotContain: []string{"tal-amuyal-fix-merchant-ax-prev-minute-request-count-returning-zeros"},
		},
		{
			name:       "long repo name is tail-truncated on a clean repo",
			innerWidth: 84,
			info: WorkspaceInfo{
				Name:     "ws",
				Category: CategoryActiveNoSession,
				RepoGitStatuses: map[string]RepoGitStatus{
					"analytics-backend-ingestion-and-dashboard-rendering-platform-service": {Branch: "main", IsClean: true},
				},
			},
			mustContain:    []string{"\u2713 clean", "\u2026"},
			mustNotContain: []string{"analytics-backend-ingestion-and-dashboard-rendering-platform-service"},
		},
		{
			name:       "narrow inner width shrinks the name column",
			innerWidth: 40,
			info: WorkspaceInfo{
				Name:     "ws",
				Category: CategoryActiveNoSession,
				RepoGitStatuses: map[string]RepoGitStatus{
					"analytics-backend-ingestion-and-dashboard-rendering-platform-service": {Branch: "HEAD", IsClean: true},
				},
			},
			mustContain:    []string{"\u2713 clean", "\u2026"},
			mustNotContain: []string{"analytics-backend-ingestion-and-dashboard-rendering-platform-service"},
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			rendered := m.renderCard(tc.info, false, tc.innerWidth)
			row := findRepoRow(t, rendered)

			for _, want := range tc.mustContain {
				if !strings.Contains(row, want) {
					t.Errorf("expected row to contain %q, got:\n%s", want, row)
				}
			}
			for _, unwanted := range tc.mustNotContain {
				if strings.Contains(row, unwanted) {
					t.Errorf("expected row NOT to contain %q, got:\n%s", unwanted, row)
				}
			}

			// The rendered card adds a 2-cell left margin outside the border.
			if rowWidth := lipgloss.Width(row); rowWidth > tc.innerWidth+2 {
				t.Errorf("repo row width %d exceeds innerWidth+margin %d:\n%s", rowWidth, tc.innerWidth+2, row)
			}
		})
	}
}
