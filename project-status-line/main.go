package main

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strconv"
)

type statusInput struct {
	Cost struct {
		TotalCostUSD float64 `json:"total_cost_usd"`
	} `json:"cost"`
	ContextWindow struct {
		UsedPercentage *float64 `json:"used_percentage"`
	} `json:"context_window"`
}

func processInput(reader io.Reader, writer io.Writer) error {
	var input statusInput
	if err := json.NewDecoder(reader).Decode(&input); err != nil {
		return err
	}

	cost := formatNumber(input.Cost.TotalCostUSD)

	contextUsage := "0"
	if input.ContextWindow.UsedPercentage != nil {
		contextUsage = strconv.FormatFloat(*input.ContextWindow.UsedPercentage, 'f', -1, 64)
	}

	fmt.Fprintf(writer, "[%s$] [%s%%]\n", cost, contextUsage)
	return nil
}

func main() {
	if err := processInput(os.Stdin, os.Stdout); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
