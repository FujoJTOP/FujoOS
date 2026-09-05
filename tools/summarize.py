#!/usr/bin/env python3
"""summarize.py — print eval_results table sorted by novel coverage."""
import glob
import json
import os

rows = []
for fn in sorted(glob.glob(os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                        "..", "eval_results", "*.json"))):
    d = json.load(open(fn))
    rows.append((d["model"], d["anom"], d["anom_t"], d["io"], d["io_t"],
                 d["cls"], d["cls_t"], d["novel_pos_hits"], d["pass"]))
print(f"{'model':18s} {'anom':8s} {'io':6s} {'cls':6s} {'novel':5s} pass")
for r in sorted(rows, key=lambda x: -x[7]):
    print(f"{r[0]:18s} {r[1]}/{r[2]:<5d} {r[3]}/{r[4]:<4d} {r[5]}/{r[6]:<4d} {r[7]:<5d} {r[8]}")
