package components

// VimNav provides vim-style navigation key handling.
// Embed in any list model that needs j/k/gg/G navigation.
type VimNav struct {
	gPressed bool
}

// HandleKey processes a vim navigation key. Returns true if the key was consumed.
// Callbacks are invoked for the corresponding navigation action.
func (v *VimNav) HandleKey(key string, goFirst func(), goLast func(), cursorUp func(), cursorDown func()) bool {
	switch key {
	case "j", "down":
		cursorDown()
		v.gPressed = false
		return true
	case "k", "up":
		cursorUp()
		v.gPressed = false
		return true
	case "g":
		if v.gPressed {
			v.gPressed = false
			goFirst()
		} else {
			v.gPressed = true
		}
		return true
	case "G":
		goLast()
		v.gPressed = false
		return true
	default:
		v.gPressed = false
		return false
	}
}

// Reset clears the pending g state. Call when focus changes.
func (v *VimNav) Reset() {
	v.gPressed = false
}
