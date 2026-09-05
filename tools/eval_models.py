#!/usr/bin/env python3
"""eval_models.py — B20: 15-model x 100-sample m141 evaluation (resumable).

Per model: verify_ai.py (QEMU + COM2/shm server + Ollama) -> parse the
[model] engine row + T3 candidate hits -> eval_results/<model>.json.
Skip models whose json exists (crash-safe resume).
"""
import glob
import json
import os
import re
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MODELS = [
    "qwen2.5:0.5b", "qwen2.5:1.5b", "qwen2.5:3b", "qwen2.5:7b",
    "qwen3:0.6b", "qwen3:1.7b", "qwen3:4b", "qwen3:8b",
    "llama3.2:1b", "llama3.2:3b", "gemma2:2b", "gemma2:9b",
    "phi3:mini", "mistral:7b", "deepseek-r1:1.5b",
]
OUT = os.path.join(ROOT, "eval_results")
os.makedirs(OUT, exist_ok=True)
TMP_PREFIX = "ai-verify-"


def parse(log):
    txt = open(log, errors="replace").read()
    m = re.search(r"m141: \[model\] anom=(\d+)/(\d+) io=(\d+)/(\d+) cls=(\d+)/(\d+)", txt)
    if not m:
        return None
    novel = re.search(r"m141: T3 model novel-pos anom (\d+)/10", txt)
    cands = []
    for l in txt.splitlines():
        if "cand anom-novel" in l or "cand cls-novel" in l:
            c = re.search(r"'([^']+)' -> (\d+) gt=(\d+) (HIT|miss)", l)
            if c:
                cands.append(list(c.groups()))
    return {
        "anom": int(m.group(1)), "anom_t": int(m.group(2)),
        "io": int(m.group(3)), "io_t": int(m.group(4)),
        "cls": int(m.group(5)), "cls_t": int(m.group(6)),
        "novel_pos_hits": int(novel.group(1)) if novel else -1,
        "cands": cands,
        "pass": "M141 RESULT: PASS" in txt,
    }


def newest_log():
    dirs = sorted(glob.glob(os.path.join(tempfile.gettempdir(), TMP_PREFIX + "*")),
                  key=os.path.getmtime)
    if not dirs:
        return ""
    cand = os.path.join(dirs[-1], "qemu.log")
    return cand if os.path.exists(cand) else ""


def preheat(model):
    """Load the model into VRAM so the first inference is fast (T0 probe)."""
    try:
        subprocess.run(["ollama", "run", model, "hi"], capture_output=True,
                       text=True, timeout=240)
        print(f"[warm] {model}", flush=True)
    except Exception as e:
        print(f"[warm-fail] {model}: {e}", flush=True)


def main():
    for model in MODELS:
        fn = os.path.join(OUT, model.replace(":", "_") + ".json")
        if os.path.exists(fn):
            print(f"[skip] {model} done", flush=True)
            continue
        preheat(model)
        print(f"[run ] {model}", flush=True)
        r = subprocess.run(
            [sys.executable, os.path.join(ROOT, "tools", "verify_ai.py"),
             "--demo", "m141_eval", "--needle", "M141 RESULT: PASS",
             "--model", model, "--timeout", "700"],
            capture_output=True, text=True)
        log = newest_log()
        data = parse(log) if log else None
        if data is None:
            print(f"[FAIL] {model} no [model] row; tail={r.stdout[-400:]}", flush=True)
            continue
        data["model"] = model
        json.dump(data, open(fn, "w"), indent=1)
        print(f"[OK  ] {model} anom={data['anom']}/{data['anom_t']} "
              f"io={data['io']}/{data['io_t']} cls={data['cls']}/{data['cls_t']} "
              f"novel={data['novel_pos_hits']}/10 pass={data['pass']}", flush=True)
    print("[done] eval_models", flush=True)


if __name__ == "__main__":
    main()
