package prstatus

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os/exec"
	"strings"

	"charm.land/lipgloss/v2"

	"devora/internal/process"
	"devora/internal/style"
)

// --- Types ---

type PRData struct {
	Number            int              `json:"number"`
	Title             string           `json:"title"`
	URL               string           `json:"url"`
	State             string           `json:"state"`
	IsDraft           bool             `json:"isDraft"`
	HeadRefName       string           `json:"headRefName"`
	ReviewDecision    string           `json:"reviewDecision"`
	AutoMergeRequest  *json.RawMessage `json:"autoMergeRequest"`
	StatusCheckRollup []CheckRun       `json:"statusCheckRollup"`
	Reviews           []Review         `json:"reviews"`
}

type CheckRun struct {
	Name       string `json:"name"`
	Context    string `json:"context"`
	Status     string `json:"status"`
	Conclusion string `json:"conclusion"`
	TypeName   string `json:"__typename"`
}

type Review struct {
	Author ReviewAuthor `json:"author"`
	State  string       `json:"state"`
}

type ReviewAuthor struct {
	Login string `json:"login"`
}

type ChecksSummary struct {
	Total         int
	Passed        int
	Failed        int
	Pending       int
	Skipped       int
	FailedChecks  []string
	PendingChecks []string
}

type ReviewsSummary struct {
	Approved         int
	ChangesRequested int
	Commented        int
	Pending          int
}

type JSONOutput struct {
	Branch  string             `json:"branch"`
	PR      *JSONOutputPR      `json:"pr,omitempty"`
	Checks  *JSONOutputChecks  `json:"checks,omitempty"`
	Reviews *JSONOutputReviews `json:"reviews,omitempty"`
}

type JSONOutputPR struct {
	Number    int    `json:"number"`
	Title     string `json:"title"`
	URL       string `json:"url"`
	State     string `json:"state"`
	Draft     bool   `json:"draft"`
	AutoMerge bool   `json:"auto_merge"`
}

type JSONOutputChecks struct {
	Total         int      `json:"total"`
	Passed        int      `json:"passed"`
	Failed        int      `json:"failed"`
	Pending       int      `json:"pending"`
	Skipped       int      `json:"skipped"`
	FailedChecks  []string `json:"failed_checks"`
	PendingChecks []string `json:"pending_checks"`
}

type JSONOutputReviews struct {
	Approved         int `json:"approved"`
	ChangesRequested int `json:"changes_requested"`
	Commented        int `json:"commented"`
	Pending          int `json:"pending"`
}

// --- Stubbable dependencies ---

var getGHPRView = defaultGetGHPRView
var getCurrentBranch = defaultGetCurrentBranch

func defaultGetGHPRView() (string, error) {
	return process.GetOutput([]string{
		"gh", "pr", "view", "--json",
		"number,title,url,state,isDraft,reviewDecision,statusCheckRollup,autoMergeRequest,reviews,headRefName",
	})
}

func defaultGetCurrentBranch() (string, error) {
	return process.GetOutput([]string{"git", "symbolic-ref", "--short", "HEAD"})
}

// --- Core logic ---

func computeChecksSummary(checks []CheckRun) ChecksSummary {
	summary := ChecksSummary{
		Total:         len(checks),
		FailedChecks:  []string{},
		PendingChecks: []string{},
	}

	for _, c := range checks {
		name := c.Name
		if name == "" {
			name = c.Context
		}

		switch c.Status {
		case "IN_PROGRESS", "QUEUED", "PENDING":
			summary.Pending++
			summary.PendingChecks = append(summary.PendingChecks, name)
			continue
		}

		switch c.Conclusion {
		case "SUCCESS":
			summary.Passed++
		case "FAILURE":
			summary.Failed++
			summary.FailedChecks = append(summary.FailedChecks, name)
		case "SKIPPED", "NEUTRAL":
			summary.Skipped++
		default:
			// Empty conclusion on COMPLETED status or unknown → Pending
			summary.Pending++
			summary.PendingChecks = append(summary.PendingChecks, name)
		}
	}

	return summary
}

func computeReviewsSummary(reviews []Review) ReviewsSummary {
	// Deduplicate: last review per author wins
	lastByAuthor := make(map[string]string)
	var order []string

	for _, r := range reviews {
		login := r.Author.Login
		if _, seen := lastByAuthor[login]; !seen {
			order = append(order, login)
		}
		lastByAuthor[login] = r.State
	}

	var summary ReviewsSummary
	for _, login := range order {
		switch lastByAuthor[login] {
		case "APPROVED":
			summary.Approved++
		case "CHANGES_REQUESTED":
			summary.ChangesRequested++
		case "COMMENTED":
			summary.Commented++
		default:
			summary.Pending++
		}
	}

	return summary
}

func isAutoMergeEnabled(autoMergeRequest *json.RawMessage) bool {
	if autoMergeRequest == nil {
		return false
	}
	raw := string(*autoMergeRequest)
	return raw != "null" && raw != ""
}

// --- Run ---

func Run(w io.Writer, jsonOutput bool) error {
	branch, err := getCurrentBranch()
	if err != nil {
		return fmt.Errorf("cannot check PR status: not on a branch")
	}

	prJSON, err := getGHPRView()
	if err != nil {
		var execErr *process.VerboseExecError
		if errors.As(err, &execErr) {
			if strings.Contains(execErr.Stderr, "no pull requests found") {
				return handleNoPR(w, branch, jsonOutput)
			}
			if errors.Is(execErr.Err, exec.ErrNotFound) {
				return fmt.Errorf("gh CLI is required for PR status. Install it: https://cli.github.com")
			}
		}
		return fmt.Errorf("fetching PR data: %w", err)
	}

	var prData PRData
	if err := json.Unmarshal([]byte(prJSON), &prData); err != nil {
		return fmt.Errorf("parsing PR data: %w", err)
	}

	checksSummary := computeChecksSummary(prData.StatusCheckRollup)
	reviewsSummary := computeReviewsSummary(prData.Reviews)
	autoMerge := isAutoMergeEnabled(prData.AutoMergeRequest)
	state := strings.ToLower(prData.State)

	if jsonOutput {
		return writeJSON(w, branch, prData, checksSummary, reviewsSummary, autoMerge, state)
	}

	return writeHuman(w, branch, prData, checksSummary, reviewsSummary, autoMerge, state)
}

func handleNoPR(w io.Writer, branch string, jsonOutput bool) error {
	if jsonOutput {
		out := JSONOutput{Branch: branch}
		data, err := json.MarshalIndent(out, "", "  ")
		if err != nil {
			return fmt.Errorf("marshaling JSON output: %w", err)
		}
		fmt.Fprintln(w, string(data))
		return nil
	}

	fmt.Fprintln(w, "PR Status")
	fmt.Fprintln(w)
	fmt.Fprintf(w, "Branch: %s\n", branch)
	fmt.Fprintf(w, "%s No PR found for branch %s\n", style.Warning.Render("\u26a0"), branch)
	return nil
}

func writeJSON(w io.Writer, branch string, prData PRData, checks ChecksSummary, reviews ReviewsSummary, autoMerge bool, state string) error {
	out := JSONOutput{
		Branch: branch,
		PR: &JSONOutputPR{
			Number:    prData.Number,
			Title:     prData.Title,
			URL:       prData.URL,
			State:     state,
			Draft:     prData.IsDraft,
			AutoMerge: autoMerge,
		},
	}

	out.Checks = &JSONOutputChecks{
		Total:         checks.Total,
		Passed:        checks.Passed,
		Failed:        checks.Failed,
		Pending:       checks.Pending,
		Skipped:       checks.Skipped,
		FailedChecks:  checks.FailedChecks,
		PendingChecks: checks.PendingChecks,
	}

	out.Reviews = &JSONOutputReviews{
		Approved:         reviews.Approved,
		ChangesRequested: reviews.ChangesRequested,
		Commented:        reviews.Commented,
		Pending:          reviews.Pending,
	}

	data, err := json.MarshalIndent(out, "", "  ")
	if err != nil {
		return fmt.Errorf("marshaling JSON output: %w", err)
	}
	fmt.Fprintln(w, string(data))
	return nil
}

func writeHuman(w io.Writer, branch string, prData PRData, checks ChecksSummary, reviews ReviewsSummary, autoMerge bool, state string) error {
	fmt.Fprintln(w, "PR Status")
	fmt.Fprintln(w)
	fmt.Fprintf(w, "Branch: %s\n", branch)
	fmt.Fprintf(w, "PR #%d: %s\n", prData.Number, prData.Title)

	// State line
	stateDisplay := renderState(state, prData.IsDraft)
	if autoMerge && state == "open" {
		stateDisplay += " (auto-merge enabled)"
	}
	fmt.Fprintf(w, "  State: %s\n", stateDisplay)

	// Checks line
	if checks.Total > 0 {
		renderChecksSection(w, checks)
	}

	// Reviews line
	renderReviewsSection(w, reviews)

	fmt.Fprintf(w, "  URL: %s\n", prData.URL)

	return nil
}

func renderState(state string, isDraft bool) string {
	switch {
	case state == "merged":
		return style.Green.Render("merged")
	case state == "closed":
		return style.Red.Render("closed")
	case isDraft:
		return style.Yellow.Render("draft")
	default:
		return style.Cyan.Render("open")
	}
}

func renderChecksSection(w io.Writer, checks ChecksSummary) {
	summaryText := fmt.Sprintf("%d/%d passing", checks.Passed, checks.Total)

	var summaryStyle lipgloss.Style
	if checks.Failed > 0 {
		summaryText += fmt.Sprintf(" (%d failed)", checks.Failed)
		summaryStyle = style.Error
	} else if checks.Pending > 0 {
		summaryText += fmt.Sprintf(" (%d pending)", checks.Pending)
		summaryStyle = style.Warning
	} else {
		summaryStyle = style.Success
	}

	fmt.Fprintf(w, "  Checks: %s\n", summaryStyle.Render(summaryText))

	const maxShown = 5

	// List failed checks
	for i, name := range checks.FailedChecks {
		if i >= maxShown {
			fmt.Fprintf(w, "    ...and %d more\n", len(checks.FailedChecks)-maxShown)
			break
		}
		fmt.Fprintf(w, "    %s %s\n", style.Error.Render("\u2717"), name)
	}

	// List pending checks
	for i, name := range checks.PendingChecks {
		if i >= maxShown {
			fmt.Fprintf(w, "    ...and %d more\n", len(checks.PendingChecks)-maxShown)
			break
		}
		fmt.Fprintf(w, "    %s %s\n", style.Warning.Render("\u25cb"), name)
	}
}

func renderReviewsSection(w io.Writer, reviews ReviewsSummary) {
	var parts []string
	if reviews.Approved > 0 {
		parts = append(parts, style.Success.Render(fmt.Sprintf("%d approved", reviews.Approved)))
	}
	if reviews.ChangesRequested > 0 {
		parts = append(parts, style.Error.Render(fmt.Sprintf("%d changes requested", reviews.ChangesRequested)))
	}
	if reviews.Pending > 0 {
		parts = append(parts, style.Warning.Render(fmt.Sprintf("%d pending", reviews.Pending)))
	}
	if len(parts) > 0 {
		fmt.Fprintf(w, "  Reviews: %s\n", strings.Join(parts, ", "))
	}
}
