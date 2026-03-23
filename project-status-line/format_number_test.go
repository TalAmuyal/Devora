package main

import "testing"

func TestFormatNumber(t *testing.T) {
	tests := []struct {
		input    float64
		expected string
	}{
		// < 1: 2 decimal places
		{0.0, "0.00"},
		{0.42, "0.42"},
		{0.99, "0.99"},
		{0.123, "0.12"},
		{0.005, "0.01"},

		// >= 1 and < 10: 1 decimal place
		{1.0, "1.0"},
		{1.23, "1.2"},
		{9.99, "10.0"},
		{5.55, "5.5"},

		// >= 10 and < 1000: no decimals
		{10.0, "10"},
		{99.9, "100"},
		{999.4, "999"},

		// >= 1000 and < 1000000: K suffix, no decimals
		{1000.0, "1K"},
		{1500.0, "2K"},
		{50000.0, "50K"},
		{999999.0, "1000K"},

		// >= 1000000: M suffix, 1 decimal
		{1000000.0, "1.0M"},
		{1500000.0, "1.5M"},
		{25000000.0, "25.0M"},
	}

	for _, tt := range tests {
		t.Run(tt.expected, func(t *testing.T) {
			result := formatNumber(tt.input)
			if result != tt.expected {
				t.Errorf("formatNumber(%v) = %q, want %q", tt.input, result, tt.expected)
			}
		})
	}
}
