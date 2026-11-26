# The report over test outcomes (flake-report.sh): input, every outcome (-s); $listed, the runs (gh run list's JSON);
# $filter, a regex on "<suite> <name>" or empty. A run counts for a test if it ran there (not only skipped);
# it failed if any attempt failed, and it "passed on re-run" if an attempt failed and its last attempt passed.
# It "needed a retry" if any attempt recorded one (`retries`, an XCUITest's own measured retry): a test that passes that way
# is listed apart, so a retry hides no flake.
# A run the UI test watchdog stopped at the test (`watchdog`, its verdict) is listed apart too: when it found the runner's
# WindowServer frozen ("runner"), the test's failure in that run isn't counted as the test's.
def median: sort | if length == 0 then null else .[length / 2 | floor] end;
def secs: if . == null then "-" elif . < 10 then (. * 10 | round / 10 | tostring) + "s" else (round | tostring) + "s" end;
($listed | map({key: (.databaseId | tostring), value: .}) | from_entries) as $runs
| map(select($filter == "" or ((.suite + " " + .name) | test($filter))))
| group_by([.job, .suite, .name])
| map(
	. as $all
	| (group_by(.run_id) | map(
		sort_by(.run_attempt)
		| {run: .[0].run_id, event: .[0].event,
			failed: (any(.[]; .outcome == "failed") and (any(.[]; .watchdog == "runner") | not)),
			passed_on_rerun: (any(.[]; .outcome == "failed") and (any(.[]; .watchdog == "runner") | not) and (last.outcome == "passed")),
			froze: any(.[]; .watchdog == "runner"),
			hung: any(.[]; .watchdog == "test"),
			retries: (map(.retries // 0) | add),
			ran: any(.[]; .outcome != "skipped")})) as $per_run
	| ($per_run | map(select(.ran))) as $ran
	| {
		job: $all[0].job, suite: $all[0].suite, name: $all[0].name,
		master: [($ran | map(select(.event == "push" and .failed)) | length), ($ran | map(select(.event == "push")) | length)],
		pr: [($ran | map(select(.event == "pull_request" and .failed)) | length), ($ran | map(select(.event == "pull_request")) | length)],
		reruns: ($per_run | map(select(.passed_on_rerun)) | length),
		retried: [($ran | map(select(.event == "push" and .retries > 0)) | length), ($ran | map(select(.event == "pull_request" and .retries > 0)) | length)],
		retries: ($per_run | map(.retries) | add),
		last_retry: ($per_run | map(select(.retries > 0)) | map($runs[.run] // {databaseId: .run}) | sort_by(.createdAt) | last),
		stops: [($per_run | map(select(.froze)) | length), ($per_run | map(select(.hung)) | length)],
		stopped_in: ($per_run | map(select(.froze or .hung)) | map(($runs[.run] // {databaseId: .run}) + {verdict: (if .froze then "runner" else "test" end)}) | sort_by(.createdAt)),
		last_failure: ($per_run | map(select(.failed)) | map($runs[.run] // {databaseId: .run}) | sort_by(.createdAt) | last),
		median: ($all | map(select(.outcome == "passed") | .duration // empty) | median),
		slowest: ($all | map(.duration // empty) | max)
	})
| (map(select(.master[0] + .pr[0] > 0)) | sort_by(-(.master[0] + .pr[0]), -.reruns)) as $failing
| (map(select(.retries > 0)) | sort_by(-(.retried[0] + .retried[1]), -.retries)) as $retried
| (map(select(.stops[0] + .stops[1] > 0)) | sort_by(-(.stops[0] + .stops[1]))) as $stopped
| "Over the last \($listed | length) CI runs (\($listed | map(select(.event == "push")) | length) on master, \($listed | map(select(.event == "pull_request")) | length) on pull requests), \(length) tests recorded:\n",
(if ($failing | length) == 0 then "No test failed." else
	($failing[] |
		"\(.master[0])/\(.master[1]) master, \(.pr[0])/\(.pr[1]) PR  \(.job): \(.suite) / \(.name)\n"
		+ (if .reruns > 0 then "    failed, then passed on re-run, in \(.reruns) run(s)\n" else "" end)
		+ "    last failed: \(.last_failure.createdAt // "?") \(.last_failure.url // .last_failure.databaseId)\n"
		+ "    duration: median \(.median | secs), slowest \(.slowest | secs)")
end),
(if ($retried | length) == 0 then empty else
	"\nTests that needed their own retry (an XCUITest's \"… retry: …\" attachment), passing or not:",
	($retried[] |
		"\(.retried[0])/\(.master[1]) master, \(.retried[1])/\(.pr[1]) PR  \(.job): \(.suite) / \(.name)\n"
		+ "    \(.retries) retries; last: \(.last_retry.createdAt // "?") \(.last_retry.url // .last_retry.databaseId)")
end),
(if ($stopped | length) == 0 then empty else
	"\nRuns the UI test watchdog stopped at a test (\"runner\": the runner's WindowServer had frozen, so the test's failure there isn't counted above):",
	($stopped[] |
		"\(.stops[0]) runner, \(.stops[1]) test  \(.job): \(.suite) / \(.name)"
		+ (.stopped_in | map("\n    \(.verdict): \(.createdAt // "?") \(.url // .databaseId)") | add))
end),
(if $filter != "" then "\nAll matching tests:", (.[] | "  \(.job): \(.suite) / \(.name): failed \(.master[0])/\(.master[1]) master, \(.pr[0])/\(.pr[1]) PR; median \(.median | secs), slowest \(.slowest | secs)") else empty end)