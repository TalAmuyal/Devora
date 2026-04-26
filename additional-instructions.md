## JSON Validation

The `debi util json-validate ...` command replaces ad-hoc `python3 -c "import json; ..."` (and similar) invocations with a self-contained Go implementation.
It supports file paths and stdin (-), and reports errors with precise line and column numbers derived from byte offsets.

Exit codes: 0 = valid, 1 = invalid, 2 = usage/file error.

When a JSON file (or string) needs to be validated, using `debi util json-validate` is the preferred method.

## YAML Validation

The `debi util yaml-validate ...` command validates YAML using a self-contained Go implementation backed by `github.com/goccy/go-yaml`, with real line and column numbers from typed parser errors.
It supports file paths and stdin (-), handles multi-document streams (`---`-separated; all documents must parse), and rejects empty/whitespace-only input as invalid for parity with `json-validate`.

Exit codes: 0 = valid, 1 = invalid, 2 = usage/file error. Errors are reported as `Invalid YAML: line X, column Y: <detail>`.

When a YAML file (or string) needs to be validated, using `debi util yaml-validate` is the preferred method.

## TOML Validation

The `debi util toml-validate ...` command validates TOML using a self-contained Go implementation backed by `github.com/BurntSushi/toml`, with line and column numbers from `toml.ParseError.Position`.
It supports file paths and stdin (-), and rejects empty/whitespace-only input as invalid for parity with `json-validate`.

Exit codes: 0 = valid, 1 = invalid, 2 = usage/file error. Errors are reported as `Invalid TOML: line X, column Y: <detail>`.

When a TOML file (or string) needs to be validated, using `debi util toml-validate` is the preferred method.
