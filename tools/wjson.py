#!/usr/bin/env python3
"""wjson.py — extract [model] row + cands from latest ai-verify qemu.log into eval_results/<model>.json"""
import glob
import json
import os
import re
import sys
import tempfile

log = sys.argv[1]
model = sys.argv[2]
txt = open(log, errors="replace").read()
m = re.search(r"m141: \[model\] anom=(\d+)/(\d+) io=(\d+)/(\d+) cls=(\d+)/(\d+)", txt)
if not m:
    print("no [model] row")
    sys.exit(1)
n = re.search(r"m141: T3 model novel-pos anom (\d+)/10", txt)
cands = []
for l in txt.splitlines():
    if "cand anom-novel" in l or "cand cls-novel" in l:
        c = re.search(r"'([^']+)' -> (\d+) gt=(\d+) (HIT|miss)", l)
        if c:
            cands.append(list(c.groups()))
d = {
    "anom": int(m.group(1)), "anom_t": int(m.group(2)),
    "io": int(m.group(3)), "io_t": int(m.group(4)),
    "cls": int(m.group(5)), "cls_t": int(m.group(6)),
    "novel_pos_hits": int(n.group(1)) if n else -1,
    "cands": cands, "pass": "M141 RESULT: PASS" in txt, "model": model,
}
out = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                   "eval_results", model.replace(":", "_") + ".json")
json.dump(d, open(out, "w"), indent=1)
print(f"written {out}: anom={d['anom']}/{d['anom_t']} io={d['io']}/{d['io_t']} "
      f"cls={d['cls']}/{d['cls_t']} novel={d['novel_pos_hits']} pass={d['pass']}")
