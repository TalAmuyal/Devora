package main

import (
	"devora/internal/cli"
	"devora/internal/crash"
	"devora/internal/process"
	"errors"
	"fmt"
	"os"
)

func main() {
	defer func() {
		if r := recover(); r != nil {
			crash.HandlePanic(r)
			os.Exit(1)
		}
	}()
	if err := cli.Run(os.Args[1:]); err != nil {
		var usageErr *cli.UsageError
		if errors.As(err, &usageErr) {
			fmt.Fprintln(os.Stderr, usageErr.Message)
			os.Exit(1)
		}
		var ptErr *process.PassthroughError
		if errors.As(err, &ptErr) {
			os.Exit(ptErr.Code)
		}
		crash.HandleError(err)
		os.Exit(1)
	}
}
