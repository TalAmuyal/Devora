package cli

import (
	"devora/internal/config"
	"devora/internal/process"
	"encoding/json"
	"fmt"
	"math"
	"os"
	"strings"
)

// configGet is the config resolution entry point; stubbable for tests.
var configGet = config.Get

const getConfUsage = `usage: debi get-conf [--profile <name>] <key>

Print the resolved value of a config key (dot-path notation).
Exits with code 1 if the key is not found.`

func runGetConf(args []string) error {
	profile := ""
	var key string

	for i := 0; i < len(args); i++ {
		arg := args[i]
		switch {
		case arg == "-h" || arg == "--help":
			fmt.Println(getConfUsage)
			return nil
		case arg == "-p" || arg == "--profile" || strings.HasPrefix(arg, "--profile="):
			val, nextI, err := parseValue(args, i, arg, "--profile")
			if err != nil {
				return err
			}
			profile = val
			i = nextI
		case strings.HasPrefix(arg, "-"):
			return &UsageError{Message: fmt.Sprintf("unknown flag: %s\n%s", arg, getConfUsage)}
		default:
			if key != "" {
				return &UsageError{Message: fmt.Sprintf("unexpected argument: %s\n%s", arg, getConfUsage)}
			}
			key = arg
		}
	}

	if key == "" {
		return &UsageError{Message: getConfUsage}
	}

	if _, err := resolveActiveProfile(profile); err != nil {
		return err
	}

	val, ok := configGet(key)
	if !ok {
		return &process.PassthroughError{Code: 1}
	}

	return printConfigValue(val)
}

func printConfigValue(val any) error {
	switch v := val.(type) {
	case string:
		fmt.Println(v)
	case bool:
		fmt.Println(v)
	case float64:
		if v == math.Trunc(v) && !math.IsInf(v, 0) {
			fmt.Printf("%d\n", int64(v))
		} else {
			fmt.Printf("%g\n", v)
		}
	default:
		enc := json.NewEncoder(os.Stdout)
		if err := enc.Encode(val); err != nil {
			return err
		}
	}
	return nil
}
