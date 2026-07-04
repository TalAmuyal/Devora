/*
Package judgess implements the `debi judgess` Claude Code permission hook.

Claude Code invokes `debi judgess` for each permission request (except ExitPlanMode, which is excluded at the matcher level).
The request is passed as JSON on stdin, and the hook reports its decision through the process exit code and stdout - see hook contract below.

Judgess currently abstains on every request: it parses the payload and exits 0 with empty stdout, so Claude Code falls through to its normal permission flow.
Per-tool allow/deny decisions will be added incrementally at the post-parse dispatch point in Run.

## Hook contract

`debi judgess` communicates its decision to Claude Code through the exit code and stdout:

| Outcome | Exit | stdout | Effect |
|---|---|---|---|
| **Abstain** (current behavior) | `0` | *empty* | No decision; Claude Code applies its normal permission flow |
| Allow (future) | `0` | `hookSpecificOutput` JSON with `decision.behavior = "allow"` | Auto-approve |
| Deny (future) | `2` | — (reason on stderr) | Auto-deny |
| Defer / error | `1` (any non-`2`, non-zero) | — | Non-blocking error; falls through to the normal flow |

Exit `2` is the **only** deny signal, so any non-decision path must avoid it.
A malformed payload is reported as exit `1` (non-blocking) — never a deny.
*/


package judgess

import (
	"encoding/json"
	"fmt"
	"io"
)

type hookInput struct {
	ToolName  string          `json:"tool_name"`
	ToolInput json.RawMessage `json:"tool_input"` // kept raw so per-tool logic can later decode it into the right shape
	Cwd       string          `json:"cwd"`
}

func parseInput(r io.Reader) (hookInput, error) {
	data, err := io.ReadAll(r)
	if err != nil {
		return hookInput{}, fmt.Errorf("read hook input: %w", err)
	}

	var in hookInput
	if err := json.Unmarshal(data, &in); err != nil {
		return hookInput{}, fmt.Errorf("parse hook input: %w", err)
	}
	return in, nil
}

// Run deliberately takes no stdout writer, so it structurally cannot emit a decision yet.
// A malformed payload returns an error, which the caller maps to a non-blocking exit code (never a deny).
// Per-tool decision logic will branch on the parsed input after the parse step.
func Run(r io.Reader) error {
	if _, err := parseInput(r); err != nil {
		return err
	}
	return nil
}
