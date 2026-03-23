package main

import (
	"bytes"
	"strings"
	"testing"
)

func TestProcessInput(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{
			name:     "basic input",
			input:    `{"model":{"display_name":"Claude"},"cost":{"total_cost_usd":0.42},"context_window":{"used_percentage":55}}`,
			expected: "[0.42$] [55%]\n",
		},
		{
			name:     "high cost",
			input:    `{"model":{"display_name":"Claude"},"cost":{"total_cost_usd":1234.5},"context_window":{"used_percentage":80}}`,
			expected: "[1K$] [80%]\n",
		},
		{
			name:     "zero cost",
			input:    `{"model":{"display_name":"Claude"},"cost":{"total_cost_usd":0},"context_window":{"used_percentage":0}}`,
			expected: "[0.00$] [0%]\n",
		},
		{
			name:     "missing context_window used_percentage defaults to 0",
			input:    `{"model":{"display_name":"Claude"},"cost":{"total_cost_usd":1.5}}`,
			expected: "[1.5$] [0%]\n",
		},
		{
			name:     "fractional context window percentage",
			input:    `{"model":{"display_name":"Claude"},"cost":{"total_cost_usd":0.01},"context_window":{"used_percentage":33.5}}`,
			expected: "[0.01$] [33.5%]\n",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			reader := strings.NewReader(tt.input)
			var buf bytes.Buffer
			err := processInput(reader, &buf)
			if err != nil {
				t.Fatalf("processInput returned error: %v", err)
			}
			if buf.String() != tt.expected {
				t.Errorf("got %q, want %q", buf.String(), tt.expected)
			}
		})
	}
}

func TestProcessInputInvalidJSON(t *testing.T) {
	reader := strings.NewReader("not json")
	var buf bytes.Buffer
	err := processInput(reader, &buf)
	if err == nil {
		t.Error("expected error for invalid JSON, got nil")
	}
}
