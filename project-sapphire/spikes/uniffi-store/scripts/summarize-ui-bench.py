#!/usr/bin/env python3
"""Prints the JSON lines written by `SapphireSpike --ui-bench` as a table."""
import json, sys

print(f"{'run':28} {'done':>4} {'deltas':>6} {'fps':>5} {'drop%':>6} {'p95fr':>6} {'p99fr':>6} {'maxfr':>6} {'lat50':>7} {'lat95':>7} {'lat99':>7} {'folds':>6} {'CPU%':>5}")
for line in open(sys.argv[1]):
    r = json.loads(line); f = r['frames']; run = r.get('run'); i = f['intervalMs']
    row = f"{r['label']:28} "
    if run:
        l = run['latencyMs']
        row += f"{'yes' if run['completed'] else 'NO':>4} {run['deltas']:6d} "
    else:
        row += f"{'':>4} {'':>6} "
    row += f"{f['averageFPS']:5.0f} {f['droppedPercent']:6.2f} {i['p95']:6.2f} {i['p99']:6.2f} {i['max']:6.1f}"
    if run:
        row += f" {l['p50']:7.2f} {l['p95']:7.2f} {l['p99']:7.2f} {run['mainActorFolds']:6d} {run['processCPUPercent']:5.0f}"
    print(row)
