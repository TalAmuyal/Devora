#!/bin/bash

# Shared test helpers for bundler test scripts.

PASS=0
FAIL=0

assert_eq() {
	local label="$1"
	local expected="$2"
	local actual="$3"
	if [ "$expected" = "$actual" ]; then
		PASS=$((PASS + 1))
		echo "  ok: $label"
	else
		FAIL=$((FAIL + 1))
		echo "  FAIL: $label"
		echo "    expected: $expected"
		echo "    actual:   $actual"
	fi
}

assert_exists() {
	local label="$1"
	local path="$2"
	if [ -e "$path" ]; then
		PASS=$((PASS + 1))
		echo "  ok: $label ($path)"
	else
		FAIL=$((FAIL + 1))
		echo "  FAIL: $label — missing: $path"
	fi
}

assert_not_exists() {
	local label="$1"
	local path="$2"
	if [ ! -e "$path" ]; then
		PASS=$((PASS + 1))
		echo "  ok: $label ($path)"
	else
		FAIL=$((FAIL + 1))
		echo "  FAIL: $label — should not exist: $path"
	fi
}

print_test_results() {
	echo ""
	echo "Results: $PASS passed, $FAIL failed"
	[ "$FAIL" -eq 0 ]
}
