package components

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"charm.land/lipgloss/v2"
)

func newTestPathPicker(t *testing.T) PathPickerModel {
	t.Helper()
	s := lipgloss.NewStyle()
	m := NewPathPickerModel(s, s, s, s, 6)
	m.Focus()
	return m
}

// setupTestDir creates a temp directory with subdirectories for testing.
// Returns the temp dir path and a cleanup function.
func setupTestDir(t *testing.T, subdirs ...string) string {
	t.Helper()
	dir := t.TempDir()
	for _, sub := range subdirs {
		err := os.MkdirAll(filepath.Join(dir, sub), 0o755)
		if err != nil {
			t.Fatalf("failed to create subdir %q: %v", sub, err)
		}
	}
	return dir
}

// --- Constructor & Initial State ---

func TestNewPathPickerModel_StartsInTypeMode(t *testing.T) {
	m := newTestPathPicker(t)
	if m.Mode() != PathPickerTypeMode {
		t.Fatal("expected Type mode after construction")
	}
}

func TestNewPathPickerModel_ValueEmpty(t *testing.T) {
	m := newTestPathPicker(t)
	if m.Value() != "" {
		t.Fatalf("expected empty value, got %q", m.Value())
	}
}

// --- Focus / Blur ---

func TestFocus_EntersTypeMode(t *testing.T) {
	s := lipgloss.NewStyle()
	m := NewPathPickerModel(s, s, s, s, 6)
	m.Focus()

	if !m.Focused {
		t.Fatal("expected Focused to be true after Focus()")
	}
	if m.Mode() != PathPickerTypeMode {
		t.Fatal("expected Type mode after Focus()")
	}
}

func TestBlur_UnsetsFocus(t *testing.T) {
	m := newTestPathPicker(t)
	m.Blur()

	if m.Focused {
		t.Fatal("expected Focused to be false after Blur()")
	}
}

func TestHandleKey_ReturnsFalseWhenBlurred(t *testing.T) {
	m := newTestPathPicker(t)
	m.Blur()

	consumed := m.HandleKey("a")
	if consumed {
		t.Fatal("expected key to not be consumed when blurred")
	}
}

// --- SetValue ---

func TestSetValue_SetsTextInput(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "alpha", "beta")

	m.SetValue(dir + "/")
	if m.Value() != dir+"/" {
		t.Fatalf("expected %q, got %q", dir+"/", m.Value())
	}
}

func TestSetValue_RefreshesListing(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "alpha", "beta")

	m.SetValue(dir + "/")
	entries := m.visibleEntries()
	if len(entries) == 0 {
		t.Fatal("expected directory entries after SetValue")
	}
}

// --- SetPlaceholder ---

func TestSetPlaceholder(t *testing.T) {
	m := newTestPathPicker(t)
	m.SetPlaceholder("Enter path...")
	if m.textInput.Placeholder != "Enter path..." {
		t.Fatalf("expected placeholder 'Enter path...', got %q", m.textInput.Placeholder)
	}
}

// --- Type Mode Key Handling ---

func TestTypeMode_PrintableCharsInserted(t *testing.T) {
	m := newTestPathPicker(t)

	m.HandleKey("/")
	m.HandleKey("t")
	m.HandleKey("m")
	m.HandleKey("p")

	if m.Value() != "/tmp" {
		t.Fatalf("expected '/tmp', got %q", m.Value())
	}
}

func TestTypeMode_BackspaceWorks(t *testing.T) {
	m := newTestPathPicker(t)
	m.SetValue("/tmp")

	m.HandleKey("backspace")
	if m.Value() != "/tm" {
		t.Fatalf("expected '/tm', got %q", m.Value())
	}
}

func TestTypeMode_CtrlL_SwitchesToBrowseMode(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "sub")
	m.SetValue(dir + "/")

	consumed := m.HandleKey("ctrl+l")
	if !consumed {
		t.Fatal("expected ctrl+l to be consumed")
	}
	if m.Mode() != PathPickerBrowseMode {
		t.Fatal("expected Browse mode after ctrl+l")
	}
}

func TestTypeMode_DownArrow_SwitchesToBrowseMode(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "sub")
	m.SetValue(dir + "/")

	consumed := m.HandleKey("down")
	if !consumed {
		t.Fatal("expected down to be consumed")
	}
	if m.Mode() != PathPickerBrowseMode {
		t.Fatal("expected Browse mode after down arrow")
	}
}

func TestTypeMode_TabNotConsumed(t *testing.T) {
	m := newTestPathPicker(t)

	consumed := m.HandleKey("tab")
	if consumed {
		t.Fatal("expected tab to pass through to parent")
	}
}

func TestTypeMode_ShiftTabNotConsumed(t *testing.T) {
	m := newTestPathPicker(t)

	consumed := m.HandleKey("shift+tab")
	if consumed {
		t.Fatal("expected shift+tab to pass through to parent")
	}
}

func TestTypeMode_EnterNotConsumed(t *testing.T) {
	m := newTestPathPicker(t)

	consumed := m.HandleKey("enter")
	if consumed {
		t.Fatal("expected enter to pass through to parent in type mode")
	}
}

func TestTypeMode_CtrlCNotConsumed(t *testing.T) {
	m := newTestPathPicker(t)

	consumed := m.HandleKey("ctrl+c")
	if consumed {
		t.Fatal("expected ctrl+c to pass through to parent")
	}
}

// --- Browse Mode Key Handling ---

func TestBrowseMode_VimNavigation(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "aaa", "bbb", "ccc")
	m.SetValue(dir + "/")
	m.HandleKey("ctrl+l") // enter browse mode

	// Cursor starts at 0 (which is "..")
	if m.cursor != 0 {
		t.Fatalf("expected cursor at 0, got %d", m.cursor)
	}

	// j moves down
	m.HandleKey("j")
	if m.cursor != 1 {
		t.Fatalf("expected cursor at 1 after j, got %d", m.cursor)
	}

	// k moves up
	m.HandleKey("k")
	if m.cursor != 0 {
		t.Fatalf("expected cursor at 0 after k, got %d", m.cursor)
	}
}

func TestBrowseMode_EnterDescendsIntoDirectory(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "parent/child")
	m.SetValue(dir + "/")
	m.HandleKey("ctrl+l") // enter browse mode

	// Move to "parent" entry (after "..")
	m.HandleKey("j") // cursor on "parent"

	consumed := m.HandleKey("enter")
	if !consumed {
		t.Fatal("expected enter to be consumed in browse mode")
	}

	// Value should now be dir/parent/
	expected := dir + "/parent/"
	if m.Value() != expected {
		t.Fatalf("expected %q, got %q", expected, m.Value())
	}
}

func TestBrowseMode_EnterOnDotDot_GoesToParent(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "child")
	childDir := filepath.Join(dir, "child")
	m.SetValue(childDir + "/")
	m.HandleKey("ctrl+l") // enter browse mode

	// Cursor at 0 is ".."
	consumed := m.HandleKey("enter")
	if !consumed {
		t.Fatal("expected enter to be consumed")
	}

	// Value should now be the parent dir
	expected := dir + "/"
	if m.Value() != expected {
		t.Fatalf("expected %q, got %q", expected, m.Value())
	}
}

func TestBrowseMode_LDescendsIntoDirectory(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "sub")
	m.SetValue(dir + "/")
	m.HandleKey("ctrl+l")
	m.HandleKey("j") // cursor on "sub"

	consumed := m.HandleKey("l")
	if !consumed {
		t.Fatal("expected l to be consumed in browse mode")
	}

	expected := dir + "/sub/"
	if m.Value() != expected {
		t.Fatalf("expected %q, got %q", expected, m.Value())
	}
}

func TestBrowseMode_RightDescendsIntoDirectory(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "sub")
	m.SetValue(dir + "/")
	m.HandleKey("ctrl+l")
	m.HandleKey("j") // cursor on "sub"

	consumed := m.HandleKey("right")
	if !consumed {
		t.Fatal("expected right to be consumed in browse mode")
	}

	expected := dir + "/sub/"
	if m.Value() != expected {
		t.Fatalf("expected %q, got %q", expected, m.Value())
	}
}

func TestBrowseMode_HGoesToParent(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "child")
	childDir := filepath.Join(dir, "child")
	m.SetValue(childDir + "/")
	m.HandleKey("ctrl+l")

	consumed := m.HandleKey("h")
	if !consumed {
		t.Fatal("expected h to be consumed")
	}

	expected := dir + "/"
	if m.Value() != expected {
		t.Fatalf("expected %q, got %q", expected, m.Value())
	}
}

func TestBrowseMode_LeftGoesToParent(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "child")
	childDir := filepath.Join(dir, "child")
	m.SetValue(childDir + "/")
	m.HandleKey("ctrl+l")

	consumed := m.HandleKey("left")
	if !consumed {
		t.Fatal("expected left to be consumed")
	}

	expected := dir + "/"
	if m.Value() != expected {
		t.Fatalf("expected %q, got %q", expected, m.Value())
	}
}

func TestBrowseMode_CtrlL_SwitchesBackToTypeMode(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "sub")
	m.SetValue(dir + "/")
	m.HandleKey("ctrl+l") // to browse
	if m.Mode() != PathPickerBrowseMode {
		t.Fatal("expected Browse mode")
	}

	m.HandleKey("ctrl+l") // back to type
	if m.Mode() != PathPickerTypeMode {
		t.Fatal("expected Type mode after second ctrl+l")
	}
}

func TestBrowseMode_PrintableCharSwitchesToTypeMode(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "sub")
	m.SetValue(dir + "/")
	m.HandleKey("ctrl+l") // to browse

	m.HandleKey("x")
	if m.Mode() != PathPickerTypeMode {
		t.Fatal("expected Type mode after typing a character in browse mode")
	}
	// The character should be appended to the text input
	if !strings.HasSuffix(m.Value(), "x") {
		t.Fatalf("expected value to end with 'x', got %q", m.Value())
	}
}

func TestBrowseMode_TabNotConsumed(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "sub")
	m.SetValue(dir + "/")
	m.HandleKey("ctrl+l")

	consumed := m.HandleKey("tab")
	if consumed {
		t.Fatal("expected tab to pass through to parent in browse mode")
	}
}

func TestBrowseMode_ShiftTabNotConsumed(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "sub")
	m.SetValue(dir + "/")
	m.HandleKey("ctrl+l")

	consumed := m.HandleKey("shift+tab")
	if consumed {
		t.Fatal("expected shift+tab to pass through to parent in browse mode")
	}
}

func TestBrowseMode_CtrlCNotConsumed(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "sub")
	m.SetValue(dir + "/")
	m.HandleKey("ctrl+l")

	consumed := m.HandleKey("ctrl+c")
	if consumed {
		t.Fatal("expected ctrl+c to pass through to parent in browse mode")
	}
}

func TestBrowseMode_UpFromTopSwitchesToTypeMode(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "sub")
	m.SetValue(dir + "/")
	m.HandleKey("ctrl+l") // browse mode, cursor at 0

	m.HandleKey("up")
	if m.Mode() != PathPickerTypeMode {
		t.Fatal("expected Type mode after pressing up from top of list")
	}
}

// --- Directory Listing ---

func TestListing_ShowsOnlyDirectories(t *testing.T) {
	dir := setupTestDir(t, "subdir")
	// Create a regular file
	f, err := os.Create(filepath.Join(dir, "file.txt"))
	if err != nil {
		t.Fatal(err)
	}
	f.Close()

	m := newTestPathPicker(t)
	m.SetValue(dir + "/")

	entries := m.visibleEntries()
	for _, e := range entries {
		if e == "file.txt" {
			t.Fatal("regular files should not appear in listing")
		}
	}
}

func TestListing_DotDotFirstEntry(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "sub")
	m.SetValue(dir + "/")

	entries := m.visibleEntries()
	if len(entries) == 0 {
		t.Fatal("expected entries")
	}
	if entries[0] != ".." {
		t.Fatalf("expected first entry to be '..', got %q", entries[0])
	}
}

func TestListing_NoDotDotAtRoot(t *testing.T) {
	m := newTestPathPicker(t)
	m.SetValue("/")

	entries := m.visibleEntries()
	for _, e := range entries {
		if e == ".." {
			t.Fatal("root directory should not have '..' entry")
		}
	}
}

func TestListing_EmptyDirectory(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t) // empty dir
	m.SetValue(dir + "/")

	entries := m.visibleEntries()
	// Should have only ".."
	if len(entries) != 1 {
		t.Fatalf("expected 1 entry (just ..), got %d: %v", len(entries), entries)
	}
	if entries[0] != ".." {
		t.Fatalf("expected '..', got %q", entries[0])
	}
}

func TestListing_NonExistentPath(t *testing.T) {
	m := newTestPathPicker(t)
	m.SetValue("/nonexistent/path/that/does/not/exist/")

	entries := m.visibleEntries()
	if len(entries) != 0 {
		t.Fatalf("expected no entries for non-existent path, got %d", len(entries))
	}
	if m.browseErr == "" {
		t.Fatal("expected browseErr to be set for non-existent path")
	}
}

func TestListing_PrefixFiltering(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "alpha", "alpha-two", "beta")

	// Path does not end with / so list parent and filter by prefix
	m.SetValue(dir + "/alpha")

	entries := m.visibleEntries()
	// Should contain ".." and entries starting with "alpha"
	for _, e := range entries {
		if e == ".." {
			continue
		}
		if !strings.HasPrefix(e, "alpha") {
			t.Fatalf("expected entries filtered by 'alpha' prefix, got %q", e)
		}
	}
	// Should NOT contain "beta"
	for _, e := range entries {
		if e == "beta" {
			t.Fatal("'beta' should be filtered out when prefix is 'alpha'")
		}
	}
}

// --- Tilde Expansion ---

func TestTildeExpansion_ListsHomeDir(t *testing.T) {
	m := newTestPathPicker(t)
	m.SetValue("~/")

	entries := m.visibleEntries()
	// Home directory should have entries (it always has at least some dirs)
	if len(entries) == 0 {
		t.Fatal("expected entries when listing ~/")
	}
}

func TestTildeExpansion_BareTilde_ListsHomeDir(t *testing.T) {
	home, err := os.UserHomeDir()
	if err != nil {
		t.Skip("cannot get home dir")
	}

	// Get the expected entries from the home directory
	homeDirEntries, err := os.ReadDir(home)
	if err != nil {
		t.Skip("cannot read home dir")
	}
	var expectedSubdir string
	for _, e := range homeDirEntries {
		if e.IsDir() && !strings.HasPrefix(e.Name(), ".") {
			expectedSubdir = e.Name()
			break
		}
	}
	if expectedSubdir == "" {
		t.Skip("no non-hidden subdirectories in home dir")
	}

	m := newTestPathPicker(t)
	m.SetValue("~")

	entries := m.visibleEntries()
	found := false
	for _, e := range entries {
		if e == expectedSubdir {
			found = true
			break
		}
	}
	if !found {
		t.Fatalf("expected home dir entry %q in listing for bare ~, got %v", expectedSubdir, entries)
	}
}

func TestSetValue_BareTilde_PreservesTildeOnBrowse(t *testing.T) {
	home, err := os.UserHomeDir()
	if err != nil {
		t.Skip("cannot get home dir")
	}

	// Find a subdirectory in home
	dirEntries, err := os.ReadDir(home)
	if err != nil {
		t.Skip("cannot read home dir")
	}
	var subdir string
	for _, e := range dirEntries {
		if e.IsDir() && !strings.HasPrefix(e.Name(), ".") {
			subdir = e.Name()
			break
		}
	}
	if subdir == "" {
		t.Skip("no non-hidden subdirectories in home dir")
	}

	m := newTestPathPicker(t)
	m.SetValue("~")
	m.HandleKey("ctrl+l") // browse mode

	// Find the subdir and navigate into it
	entries := m.visibleEntries()
	targetIdx := -1
	for i, e := range entries {
		if e == subdir {
			targetIdx = i
			break
		}
	}
	if targetIdx == -1 {
		t.Skipf("subdir %q not found in entries", subdir)
	}
	for i := 0; i < targetIdx; i++ {
		m.HandleKey("j")
	}
	m.HandleKey("enter")

	// Value should preserve tilde prefix
	expected := "~/" + subdir + "/"
	if m.Value() != expected {
		t.Fatalf("expected %q (tilde preserved after bare ~), got %q", expected, m.Value())
	}
}

func TestTildePreservation_AfterBrowse(t *testing.T) {
	m := newTestPathPicker(t)
	home, err := os.UserHomeDir()
	if err != nil {
		t.Skip("cannot get home dir")
	}

	// Find a subdirectory in home
	dirEntries, err := os.ReadDir(home)
	if err != nil {
		t.Skip("cannot read home dir")
	}
	var subdir string
	for _, e := range dirEntries {
		if e.IsDir() && !strings.HasPrefix(e.Name(), ".") {
			subdir = e.Name()
			break
		}
	}
	if subdir == "" {
		t.Skip("no non-hidden subdirectories in home dir")
	}

	m.SetValue("~/")
	m.HandleKey("ctrl+l") // browse mode

	// Find the subdir in entries and navigate to it
	entries := m.visibleEntries()
	targetIdx := -1
	for i, e := range entries {
		if e == subdir {
			targetIdx = i
			break
		}
	}
	if targetIdx == -1 {
		t.Skipf("subdir %q not found in entries", subdir)
	}

	// Navigate to target
	for i := 0; i < targetIdx; i++ {
		m.HandleKey("j")
	}
	m.HandleKey("enter")

	// Value should preserve tilde
	expected := "~/" + subdir + "/"
	if m.Value() != expected {
		t.Fatalf("expected %q (tilde preserved), got %q", expected, m.Value())
	}
}

// --- View ---

func TestView_Blurred_ShowsOnlyTextInput(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "sub")
	m.SetValue(dir + "/")
	m.Blur()

	view := m.View()
	// Should NOT contain directory entries
	if strings.Contains(view, "sub") {
		t.Fatal("blurred view should not show directory entries")
	}
}

func TestView_Focused_ShowsBrowser(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "visible_dir")
	m.SetValue(dir + "/")

	view := m.View()
	if !strings.Contains(view, "visible_dir") {
		t.Fatalf("focused view should show directory entries, got:\n%s", view)
	}
}

func TestView_BrowseMode_ShowsCursor(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "sub")
	m.SetValue(dir + "/")
	m.HandleKey("ctrl+l")

	view := m.View()
	if !strings.Contains(view, "\u25b8") { // ▸
		t.Fatalf("browse mode should show cursor indicator, got:\n%s", view)
	}
}

func TestView_TypeMode_NoBrowserCursor(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "sub")
	m.SetValue(dir + "/")

	view := m.View()
	if strings.Contains(view, "\u25b8") { // ▸
		t.Fatal("type mode should not show browser cursor indicator")
	}
}

func TestView_NonExistentPath_ShowsError(t *testing.T) {
	m := newTestPathPicker(t)
	m.SetValue("/nonexistent/path/xyz/")

	view := m.View()
	// Should show some error indication
	if !strings.Contains(strings.ToLower(view), "not found") && !strings.Contains(strings.ToLower(view), "no such") {
		// Check that browseErr is set (the view may render it)
		if m.browseErr == "" {
			t.Fatal("expected error message for non-existent path")
		}
	}
}

// --- Symlinks ---

func TestListing_IncludesSymlinksToDirectories(t *testing.T) {
	dir := setupTestDir(t, "realdir")
	// Create a symlink to a directory
	linkPath := filepath.Join(dir, "linkdir")
	err := os.Symlink(filepath.Join(dir, "realdir"), linkPath)
	if err != nil {
		t.Skip("cannot create symlink")
	}

	m := newTestPathPicker(t)
	m.SetValue(dir + "/")

	entries := m.visibleEntries()
	hasLink := false
	for _, e := range entries {
		if e == "linkdir" {
			hasLink = true
			break
		}
	}
	if !hasLink {
		t.Fatalf("expected symlink 'linkdir' in entries, got %v", entries)
	}
}

// --- Wrapping ---

func TestBrowseMode_CursorWrapsDown(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "aaa")
	m.SetValue(dir + "/")
	m.HandleKey("ctrl+l")

	// Entries: ["..", "aaa"] — 2 entries
	// Move down twice to wrap
	m.HandleKey("j") // cursor 1
	m.HandleKey("j") // cursor wraps to 0

	if m.cursor != 0 {
		t.Fatalf("expected cursor to wrap to 0, got %d", m.cursor)
	}
}

func TestBrowseMode_GG_GoesFirst(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "aaa", "bbb", "ccc")
	m.SetValue(dir + "/")
	m.HandleKey("ctrl+l")
	m.HandleKey("j")
	m.HandleKey("j") // cursor at 2

	m.HandleKey("g")
	m.HandleKey("g") // gg
	if m.cursor != 0 {
		t.Fatalf("expected cursor at 0 after gg, got %d", m.cursor)
	}
}

func TestBrowseMode_ShiftG_GoesLast(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "aaa", "bbb", "ccc")
	m.SetValue(dir + "/")
	m.HandleKey("ctrl+l")

	m.HandleKey("G")
	entries := m.visibleEntries()
	expected := len(entries) - 1
	if m.cursor != expected {
		t.Fatalf("expected cursor at %d after G, got %d", expected, m.cursor)
	}
}

// --- Down from Type Mode enters Browse Mode with cursor at first entry ---

func TestTypeMode_DownEntersBrowseAtFirstEntry(t *testing.T) {
	m := newTestPathPicker(t)
	dir := setupTestDir(t, "sub")
	m.SetValue(dir + "/")

	m.HandleKey("down")
	if m.cursor != 0 {
		t.Fatalf("expected cursor at 0 when entering browse mode, got %d", m.cursor)
	}
}

// --- Empty value in browse mode ---

func TestBrowseMode_EmptyValue_NoEntries(t *testing.T) {
	m := newTestPathPicker(t)
	// Empty value, try to enter browse mode
	consumed := m.HandleKey("ctrl+l")
	if !consumed {
		t.Fatal("expected ctrl+l to be consumed")
	}
	// Should be in browse mode but with no entries
	entries := m.visibleEntries()
	if len(entries) != 0 {
		t.Fatalf("expected no entries for empty path, got %v", entries)
	}
}
