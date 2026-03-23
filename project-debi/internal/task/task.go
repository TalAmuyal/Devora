package task

import (
	"crypto/rand"
	"encoding/json"
	"fmt"
	"os"
	"time"
)

type taskData struct {
	UID       string `json:"uid"`
	Title     string `json:"title"`
	StartedAt string `json:"started_at"`
}

func Create(title string, workspaceTaskPath string) error {
	uid, err := generateUUIDv4()
	if err != nil {
		return fmt.Errorf("generate UUID: %w", err)
	}

	data := taskData{
		UID:       uid,
		Title:     title,
		StartedAt: time.Now().Format("2006-01-02"),
	}

	return writeTaskFile(workspaceTaskPath, data)
}

func UpdateTitle(newTitle string, workspaceTaskPath string) error {
	content, err := os.ReadFile(workspaceTaskPath)
	if err != nil {
		return fmt.Errorf("read task file: %w", err)
	}

	var data taskData
	if err := json.Unmarshal(content, &data); err != nil {
		return fmt.Errorf("parse task JSON: %w", err)
	}

	data.Title = newTitle

	return writeTaskFile(workspaceTaskPath, data)
}

func writeTaskFile(path string, data taskData) error {
	content, err := json.MarshalIndent(data, "", "    ")
	if err != nil {
		return fmt.Errorf("marshal task JSON: %w", err)
	}

	if err := os.WriteFile(path, content, 0644); err != nil {
		return fmt.Errorf("write task file: %w", err)
	}

	return nil
}

func generateUUIDv4() (string, error) {
	var uuid [16]byte
	if _, err := rand.Read(uuid[:]); err != nil {
		return "", err
	}
	uuid[6] = (uuid[6] & 0x0f) | 0x40 // version 4
	uuid[8] = (uuid[8] & 0x3f) | 0x80 // variant 10
	return fmt.Sprintf("%08x-%04x-%04x-%04x-%012x",
		uuid[0:4], uuid[4:6], uuid[6:8], uuid[8:10], uuid[10:16]), nil
}
