#!/usr/bin/env python3
"""repeat_eval.py — G7: m141 [model] repeat variance (3 key models x 2 runs).
Runs verify_ai (QEMU+server+model) per repeat, parses [model] row into
D:\\\\Dev\\\\FujoOS-private\\\\eval_results\\\\repeat\\\\<model>_r<N>.json.
Serially; never run in parallel with fujoregress (MON_PORT 4568 conflict).
"""
import glob
import json
import os
import re
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = r"D:\Dev\FujoOS-private\eval_results\repeat"
MODELS = ["qwen2.5:7b", "qwen3:8b", "qwen3:0.6b"]
REPS = 2
os.makedirs(OUT, exist_ok=True)


def newest():
    d = sorted(glob.glob(os.path.join(tempfile.gettempdir(), "ai-verify-*")),
               key=os.path.getmtime)
    return os.path.join(d[-1], "qemu.log") if d else ""


def parse(log):
    txt = open(log, errors="replace").read()
    m = re.search(r"m141: \[model\] anom=(\d+)/(\d+) io=(\d+)/(\d+) cls=(\d+)/(\d+)", txt)
    if not m:
        return None
    n = re.search(r"m141: T3 model novel-pos anom (\d+)/10", txt)
    return {"anom": int(m.group(1)), "io": int(m.group(3)), "cls": int(m.group(5)),
            "novel_pos_hits": int(n.group(1)) if n else -1,
            "pass": "M141 RESULT: PASS" in txt}


def main():
    for model in MODELS:
        for r in range(1, REPS + 1):
            fn = os.path.join(OUT, model.replace(":", "_") + f"_r{r}.json")
            if os.path.exists(fn):
                print(f"[skip] {model} r{r}", flush=True)
                continue
            subprocess.run(["ollama", "run", model, "hi"], capture_output=True, timeout=240)
            print(f"[run ] {model} r{r}", flush=True)
            subprocess.run(
                [sys.executable, os.path.join(ROOT, "tools", "verify_ai.py"),
                 "--demo", "m141_eval", "--needle", "M141 RESULT: PASS",
                 "--model", model, "--timeout", "1200"],
                capture_output=True, text=True)
            log = newest()
            d = parse(log) if log else None
            if d is None:
                print(f"[FAIL] {model} r{r}", flush=True)
                continue
            d["model"] = f"{model} r{r}"
            json.dump(d, open(fn, "w"), indent=1)
            print(f"[OK  ] {model} r{r}: anom={d['anom']}/40 io={d['io']}/30 "
                  f"cls={d['cls']}/30 novel={d['novel_pos_hits']}/10", flush=True)
    print("[done] repeat_eval", flush=True)


if __name__ == "__main__":
    main()
