// Package shellinit materializes the shell integration shipped inside the Devora app bundle.
// Its only job today is to write "shims": one tiny executable per Debi git shortcut that forwards to `debi <name>`, so that typing the bare shortcut in any session shell (whose PATH includes the shim directory) runs the Debi command.
package shellinit

import (
	"devora/internal/cmdinfo"
	"fmt"
	"os"
	"path/filepath"
)

// GitShortcutsGroup is the registry group whose commands are exposed as bare-command shims inside Devora-Ember session shells.
const GitShortcutsGroup = "Git Shortcuts"

// shimContent returns the shell script body for a shim that forwards to `debi <name>`, preserving all arguments.
func shimContent(name string) string {
	return fmt.Sprintf("#!/bin/sh\nexec debi %s \"$@\"\n", name)
}

// WriteShims writes an executable shim into dir for each command in the GitShortcutsGroup.
// dir is created if it does not exist.
// Each shim invokes `debi <name>`, relying on `debi` being on PATH at run time.
func WriteShims(dir string, cmds []cmdinfo.Command) error {
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return fmt.Errorf("create shim dir: %w", err)
	}

	for _, cmd := range cmds {
		if cmd.Group != GitShortcutsGroup {
			continue
		}
		path := filepath.Join(dir, cmd.Name)
		if err := os.WriteFile(path, []byte(shimContent(cmd.Name)), 0o755); err != nil {
			return fmt.Errorf("write shim %s: %w", cmd.Name, err)
		}
		// Chmod explicitly so the executable bits survive a restrictive umask.
		if err := os.Chmod(path, 0o755); err != nil {
			return fmt.Errorf("chmod shim %s: %w", cmd.Name, err)
		}
	}

	return nil
}
