## JSON Validation

The `debi util json-validate ...` command replaces ad-hoc `python3 -c "import json; ..."` (and similar) invocations with a self-contained Go implementation.
It supports file paths and stdin (-), and reports errors with precise line and column numbers derived from byte offsets.

Exit codes: 0 = valid, 1 = invalid, 2 = usage/file error.

When a JSON file (or string) needs to be validated, using `debi util json-validate` is the preferred method.

TEST
