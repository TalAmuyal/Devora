package task

import (
	"encoding/json"
	"os"
	"path/filepath"
	"regexp"
	"testing"
	"time"
)

func TestCreate_WritesValidJSON(t *testing.T) {
	dir := t.TempDir()
	taskPath := filepath.Join(dir, "task.json")

	err := Create("implement auth flow", taskPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	content, err := os.ReadFile(taskPath)
	if err != nil {
		t.Fatalf("read task file: %v", err)
	}

	var data map[string]string
	if err := json.Unmarshal(content, &data); err != nil {
		t.Fatalf("invalid JSON: %v", err)
	}

	if data["title"] != "implement auth flow" {
		t.Fatalf("expected title %q, got %q", "implement auth flow", data["title"])
	}
}

func TestCreate_HasValidUUID(t *testing.T) {
	dir := t.TempDir()
	taskPath := filepath.Join(dir, "task.json")

	err := Create("test task", taskPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	content, _ := os.ReadFile(taskPath)
	var data map[string]string
	json.Unmarshal(content, &data)

	uid := data["uid"]
	uuidV4Pattern := regexp.MustCompile(`^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$`)
	if !uuidV4Pattern.MatchString(uid) {
		t.Fatalf("uid %q is not a valid UUID v4", uid)
	}
}

func TestCreate_HasTodaysDate(t *testing.T) {
	dir := t.TempDir()
	taskPath := filepath.Join(dir, "task.json")

	err := Create("test task", taskPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	content, _ := os.ReadFile(taskPath)
	var data map[string]string
	json.Unmarshal(content, &data)

	expected := time.Now().Format("2006-01-02")
	if data["started_at"] != expected {
		t.Fatalf("expected started_at %q, got %q", expected, data["started_at"])
	}
}

func TestCreate_FourSpaceIndentation(t *testing.T) {
	dir := t.TempDir()
	taskPath := filepath.Join(dir, "task.json")

	err := Create("test task", taskPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	content, _ := os.ReadFile(taskPath)
	contentStr := string(content)

	// 4-space indented JSON has lines starting with "    "
	if !regexp.MustCompile(`(?m)^    "`).MatchString(contentStr) {
		t.Fatalf("expected 4-space indentation in JSON, got:\n%s", contentStr)
	}
}

func TestUpdateTitle_ChangesTitle(t *testing.T) {
	dir := t.TempDir()
	taskPath := filepath.Join(dir, "task.json")

	Create("original title", taskPath)

	err := UpdateTitle("new title", taskPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	content, _ := os.ReadFile(taskPath)
	var data map[string]string
	json.Unmarshal(content, &data)

	if data["title"] != "new title" {
		t.Fatalf("expected title %q, got %q", "new title", data["title"])
	}
}

func TestUpdateTitle_PreservesOtherFields(t *testing.T) {
	dir := t.TempDir()
	taskPath := filepath.Join(dir, "task.json")

	Create("original title", taskPath)

	// Read original fields
	content, _ := os.ReadFile(taskPath)
	var original map[string]string
	json.Unmarshal(content, &original)

	err := UpdateTitle("new title", taskPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	content, _ = os.ReadFile(taskPath)
	var updated map[string]string
	json.Unmarshal(content, &updated)

	if updated["uid"] != original["uid"] {
		t.Fatalf("expected uid %q to be preserved, got %q", original["uid"], updated["uid"])
	}
	if updated["started_at"] != original["started_at"] {
		t.Fatalf("expected started_at %q to be preserved, got %q", original["started_at"], updated["started_at"])
	}
}

func TestUpdateTitle_PreservesFourSpaceIndentation(t *testing.T) {
	dir := t.TempDir()
	taskPath := filepath.Join(dir, "task.json")

	Create("original title", taskPath)
	UpdateTitle("new title", taskPath)

	content, _ := os.ReadFile(taskPath)
	if !regexp.MustCompile(`(?m)^    "`).MatchString(string(content)) {
		t.Fatalf("expected 4-space indentation in JSON, got:\n%s", content)
	}
}

func TestUpdateTitle_FileNotFound(t *testing.T) {
	err := UpdateTitle("new title", "/nonexistent/task.json")
	if err == nil {
		t.Fatal("expected error for nonexistent file")
	}
}

func TestCreate_UniqueUIDs(t *testing.T) {
	dir := t.TempDir()
	path1 := filepath.Join(dir, "task1.json")
	path2 := filepath.Join(dir, "task2.json")

	Create("task 1", path1)
	Create("task 2", path2)

	content1, _ := os.ReadFile(path1)
	content2, _ := os.ReadFile(path2)

	var data1, data2 map[string]string
	json.Unmarshal(content1, &data1)
	json.Unmarshal(content2, &data2)

	if data1["uid"] == data2["uid"] {
		t.Fatalf("expected unique UIDs, got %q for both", data1["uid"])
	}
}
