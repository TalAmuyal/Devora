#!/bin/bash
# Read JSON input once
input=$(cat)

# Check if jq is installed
if ! command -v jq &> /dev/null; then
	echo "jq is required but not installed. Please install jq to use this script."
	exit 0
fi

# Helper functions for common extractions
get_model_name() { echo "$input" | jq -r '.model.display_name'; }
get_cost() { echo "$input" | jq -r '.cost.total_cost_usd'; }
get_context_window_used_pct() { echo "$input" | jq -r '.context_window.used_percentage // 0'; }

# Helper function to formatting
format_number() {
	local number=$1
	if (( $(echo "$number < 1" | bc -l) )); then
		printf "%.2f" "$number"
	elif (( $(echo "$number < 10" | bc -l) )); then
		printf "%.1f" "$number"
	elif (( $(echo "$number < 1000" | bc -l) )); then
		printf "%.0f" "$number"
	elif (( $(echo "$number < 1000000" | bc -l) )); then
		printf "%.0fK" "$(echo "$number/1000" | bc -l)"
	else
		printf "%.1fM" "$(echo "$number/1000000" | bc -l)"
	fi
}

# Use the helpers
MODEL=$(get_model_name)

COST=$(format_number $(get_cost))

CONTEXT_WINDOW_USAGE=$(get_context_window_used_pct)

echo "[$COST\$] [$CONTEXT_WINDOW_USAGE%]"
