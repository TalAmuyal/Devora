package main

import "fmt"

func formatNumber(n float64) string {
	switch {
	case n < 1:
		return fmt.Sprintf("%.2f", n)
	case n < 10:
		return fmt.Sprintf("%.1f", n)
	case n < 1000:
		return fmt.Sprintf("%.0f", n)
	case n < 1000000:
		return fmt.Sprintf("%.0fK", n/1000)
	default:
		return fmt.Sprintf("%.1fM", n/1000000)
	}
}
