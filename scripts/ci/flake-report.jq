# The report over test outcomes (flake-report.sh): input, every outcome (-s); $listed, the runs (gh run list's JSON);
# $filter, a regex on "<suite> <name>" or empty. A run counts for a test if it ran there (not only skipped);
# it failed if any attempt failed, and it "passed on re-run" if an attempt failed and its last attempt passed.
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
			failed: any(.[]; .outcome == "failed"),
			passed_on_rerun: (any(.[]; .outcome == "failed") and (last.outcome == "passed")),
			ran: any(.[]; .outcome != "skipped")})) as $per_run
	| ($per_run | map(select(.ran))) as $ran
	| {
		job: $all[0].job, suite: $all[0].suite, name: $all[0].name,
		master: [($ran | map(select(.event == "push" and .failed)) | length), ($ran | map(select(.event == "push")) | length)],
		pr: [($ran | map(select(.event == "pull_request" and .failed)) | length), ($ran | map(select(.event == "pull_request")) | length)],
		reruns: ($per_run | map(select(.passed_on_rerun)) | length),
		last_failure: ($per_run | map(select(.failed)) | map($runs[.run] // {databaseId: .run}) | sort_by(.createdAt) | last),
		median: ($all | map(select(.outcome == "passed") | .duration // empty) | median),
		slowest: ($all | map(.duration // empty) | max)
	})
| (map(select(.master[0] + .pr[0] > 0)) | sort_by(-(.master[0] + .pr[0]), -.reruns)) as $failing
| "Over the last \($listed | length) CI runs (\($listed | map(select(.event == "push")) | length) on master, \($listed | map(select(.event == "pull_request")) | length) on pull requests), \(length) tests recorded:\n",
(if ($failing | length) == 0 then "No test failed." else
	($failing[] |
		"\(.master[0])/\(.master[1]) master, \(.pr[0])/\(.pr[1]) PR  \(.job): \(.suite) / \(.name)\n"
		+ (if .reruns > 0 then "    failed, then passed on re-run, in \(.reruns) run(s)\n" else "" end)
		+ "    last failed: \(.last_failure.createdAt // "?") \(.last_failure.url // .last_failure.databaseId)\n"
		+ "    duration: median \(.median | secs), slowest \(.slowest | secs)")
end),
(if $filter != "" then "\nAll matching tests:", (.[] | "  \(.job): \(.suite) / \(.name): failed \(.master[0])/\(.master[1]) master, \(.pr[0])/\(.pr[1]) PR; median \(.median | secs), slowest \(.slowest | secs)") else empty end)