# The jobs GitHub cancelled before a runner took them, from flake-report.sh: those it found no runner for, by job and runner
# label (a pool short of capacity), apart from those a newer run of the same branch replaced, which is the workflow's own
# concurrency rule and nothing to fix.
def no_runner: .reason | startswith("The job was not acquired");
def replaced: .reason | startswith("Canceling since a higher priority waiting request");
(map(select(no_runner)) | if length == 0 then empty else
	"Jobs GitHub found no runner for (capacity, not tests):",
	(group_by([.job, .label])[] | sort_by(.run | tonumber) | "  \(.[0].job) on \(.[0].label): \(length) time(s), last \(last.url)")
end),
(map(select(replaced)) | if length == 0 then empty else "Jobs a newer run of the same branch replaced before they started: \(length)" end),
(map(select((no_runner or replaced) | not)) | if length == 0 then empty else
	"Jobs cancelled before they started, for another reason:",
	(.[] | "  \(.job) on \(.label): \(if .reason == "" then "(no annotation)" else .reason end) \(.url)")
end)
