package components

import (
	"os"
	"path/filepath"
	"strings"
	"unicode"

	"charm.land/lipgloss/v2"
)

// PathPickerMode represents whether the user is typing or browsing.
type PathPickerMode int

const (
	PathPickerTypeMode   PathPickerMode = iota
	PathPickerBrowseMode
)

// PathPickerModel bundles a text input with a directory browser for picking directory paths.
type PathPickerModel struct {
	textInput TextInputModel
	mode      PathPickerMode
	Focused   bool

	// Browser state
	entries    []string // directory names in the currently listed dir
	cursor     int
	browseBase string // the resolved directory whose contents are listed
	browseErr  string // error message if listing failed
	prefix     string // prefix filter for partial path (basename when path doesn't end with /)
	vim        VimNav

	// Whether the original typed value started with ~/
	tildePrefix bool

	// Configuration
	browserHeight int

	// Styles
	mutedStyle  lipgloss.Style
	cursorStyle lipgloss.Style
}

func NewPathPickerModel(focusedStyle, blurredStyle, mutedStyle, cursorStyle lipgloss.Style, browserHeight int) PathPickerModel {
	return PathPickerModel{
		textInput:     NewTextInputModel(focusedStyle, blurredStyle),
		mode:          PathPickerTypeMode,
		browserHeight: browserHeight,
		mutedStyle:    mutedStyle,
		cursorStyle:   cursorStyle,
	}
}

func (m *PathPickerModel) Focus() {
	m.Focused = true
	m.mode = PathPickerTypeMode
	m.textInput.Focus()
	m.vim.Reset()
}

func (m *PathPickerModel) Blur() {
	m.Focused = false
	m.textInput.Blur()
	m.vim.Reset()
}

func (m *PathPickerModel) SetValue(value string) {
	m.textInput.SetValue(value)
	m.updateTildePrefix()
	m.refreshListing()
}

func (m *PathPickerModel) Value() string {
	return m.textInput.Value
}

func (m *PathPickerModel) Mode() PathPickerMode {
	return m.mode
}

func (m *PathPickerModel) SetPlaceholder(placeholder string) {
	m.textInput.Placeholder = placeholder
}

// HandleKey processes a key press. Returns true if consumed.
func (m *PathPickerModel) HandleKey(key string) bool {
	if !m.Focused {
		return false
	}
	if m.mode == PathPickerBrowseMode {
		return m.handleBrowseKey(key)
	}
	return m.handleTypeKey(key)
}

func (m *PathPickerModel) handleTypeKey(key string) bool {
	switch key {
	case "ctrl+l", "down":
		m.mode = PathPickerBrowseMode
		m.cursor = 0
		m.vim.Reset()
		m.refreshListing()
		return true
	}

	consumed := m.textInput.HandleKey(key)
	if consumed {
		m.updateTildePrefix()
		m.refreshListing()
	}
	return consumed
}

func (m *PathPickerModel) handleBrowseKey(key string) bool {
	switch key {
	case "ctrl+l":
		m.mode = PathPickerTypeMode
		m.textInput.Focus()
		return true

	case "tab", "shift+tab", "ctrl+c":
		return false

	case "enter", "l", "right":
		m.selectEntry()
		return true

	case "h", "left":
		m.goToParent()
		return true

	case "up", "k":
		if m.cursor == 0 {
			m.mode = PathPickerTypeMode
			m.textInput.Focus()
			return true
		}
		m.cursorUp()
		return true

	case "down", "j":
		m.cursorDown()
		return true
	}

	// gg / G via VimNav
	if consumed := m.vim.HandleKey(key, m.goFirst, m.goLast, m.cursorUp, m.cursorDown); consumed {
		return true
	}

	// Printable character: switch to type mode and insert
	keyRunes := []rune(key)
	if len(keyRunes) == 1 && unicode.IsPrint(keyRunes[0]) {
		m.mode = PathPickerTypeMode
		m.textInput.Focus()
		m.textInput.HandleKey(key)
		m.updateTildePrefix()
		m.refreshListing()
		return true
	}

	return false
}

func (m *PathPickerModel) cursorDown() {
	if len(m.entries) == 0 {
		return
	}
	m.cursor++
	if m.cursor >= len(m.entries) {
		m.cursor = 0
	}
}

func (m *PathPickerModel) cursorUp() {
	if len(m.entries) == 0 {
		return
	}
	m.cursor--
	if m.cursor < 0 {
		m.cursor = len(m.entries) - 1
	}
}

func (m *PathPickerModel) goFirst() {
	m.cursor = 0
}

func (m *PathPickerModel) goLast() {
	if len(m.entries) == 0 {
		return
	}
	m.cursor = len(m.entries) - 1
}

func (m *PathPickerModel) selectEntry() {
	if len(m.entries) == 0 {
		return
	}
	entry := m.entries[m.cursor]
	if entry == ".." {
		m.goToParent()
		return
	}

	newPath := filepath.Join(m.browseBase, entry) + "/"
	m.setValuePreservingTilde(newPath)
	m.cursor = 0
	m.refreshListing()
}

func (m *PathPickerModel) goToParent() {
	if m.browseBase == "" || m.browseBase == "/" {
		return
	}
	parent := filepath.Dir(m.browseBase)
	if parent == m.browseBase {
		return
	}
	newPath := parent + "/"
	m.setValuePreservingTilde(newPath)
	m.cursor = 0
	m.refreshListing()
}

func (m *PathPickerModel) updateTildePrefix() {
	m.tildePrefix = strings.HasPrefix(m.textInput.Value, "~/") || m.textInput.Value == "~"
}

func (m *PathPickerModel) setValuePreservingTilde(absPath string) {
	if m.tildePrefix {
		home, err := os.UserHomeDir()
		if err == nil && strings.HasPrefix(absPath, home) {
			absPath = "~" + absPath[len(home):]
		}
	}
	m.textInput.SetValue(absPath)
}

func (m *PathPickerModel) refreshListing() {
	m.entries = nil
	m.browseErr = ""
	m.browseBase = ""
	m.prefix = ""

	value := m.textInput.Value
	if value == "" {
		return
	}

	expanded := m.expandTilde(value)

	var listDir string
	if strings.HasSuffix(expanded, "/") {
		listDir = expanded
	} else {
		listDir = filepath.Dir(expanded)
		m.prefix = filepath.Base(expanded)
	}

	listDir = filepath.Clean(listDir)
	m.browseBase = listDir

	dirEntries, err := os.ReadDir(listDir)
	if err != nil {
		m.browseErr = "Directory not found"
		return
	}

	// Add ".." unless at root
	if listDir != "/" {
		m.entries = append(m.entries, "..")
	}

	for _, entry := range dirEntries {
		// Skip hidden entries
		if strings.HasPrefix(entry.Name(), ".") {
			continue
		}

		isDir := entry.IsDir()
		if !isDir && entry.Type()&os.ModeSymlink != 0 {
			// Follow symlink to check if target is a directory
			target, err := os.Stat(filepath.Join(listDir, entry.Name()))
			if err == nil && target.IsDir() {
				isDir = true
			}
		}

		if !isDir {
			continue
		}

		// Apply prefix filter
		if m.prefix != "" && !strings.HasPrefix(entry.Name(), m.prefix) {
			continue
		}

		m.entries = append(m.entries, entry.Name())
	}

	// Clamp cursor
	if m.cursor >= len(m.entries) {
		if len(m.entries) > 0 {
			m.cursor = len(m.entries) - 1
		} else {
			m.cursor = 0
		}
	}
}

func (m *PathPickerModel) expandTilde(path string) string {
	if path == "~" {
		home, err := os.UserHomeDir()
		if err != nil {
			return path
		}
		return home + "/"
	}
	if strings.HasPrefix(path, "~/") {
		home, err := os.UserHomeDir()
		if err != nil {
			return path
		}
		rest := path[2:]
		if rest == "" {
			return home + "/"
		}
		expanded := filepath.Join(home, rest)
		if strings.HasSuffix(path, "/") {
			expanded += "/"
		}
		return expanded
	}
	return path
}

// visibleEntries returns the current entries for testing.
func (m *PathPickerModel) visibleEntries() []string {
	return m.entries
}

// View renders the path picker.
func (m *PathPickerModel) View() string {
	var b strings.Builder

	// Text input line
	b.WriteString(m.textInput.View())

	if !m.Focused {
		return b.String()
	}

	// Browser section
	if m.browseErr != "" {
		b.WriteString("\n")
		b.WriteString(m.mutedStyle.Render(m.browseErr))
		return b.String()
	}

	if len(m.entries) == 0 {
		return b.String()
	}

	// Separator
	b.WriteString("\n")
	b.WriteString(m.mutedStyle.Render(strings.Repeat("\u2504", 25)))

	// Directory entries
	maxVisible := m.browserHeight
	if maxVisible <= 0 {
		maxVisible = 6
	}

	start := 0
	if m.cursor >= maxVisible {
		start = m.cursor - maxVisible + 1
	}
	end := start + maxVisible
	if end > len(m.entries) {
		end = len(m.entries)
	}

	for i := start; i < end; i++ {
		b.WriteString("\n")
		entry := m.entries[i]

		displayName := entry
		if entry != ".." {
			displayName = entry + "/"
		}

		if m.mode == PathPickerBrowseMode && i == m.cursor {
			b.WriteString(m.cursorStyle.Render("\u25b8") + " " + displayName)
		} else {
			b.WriteString("  " + m.mutedStyle.Render(displayName))
		}
	}

	return b.String()
}
