#!/usr/bin/env python3
"""plot_models.py — B20: 15-model x 100-sample quality figure (2x2 + scale group).
Usage: uv run --with matplotlib python tools/plot_models.py
Output: docs/quality-curve-15.png (private copy in D:\\Dev\\FujoOS-private\\docs)
"""
import glob
import json
import os

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FAM = ["qwen2.5", "qwen3", "llama3.2", "gemma2", "phi3", "mistral",
       "deepseek-r1"]
COL = {"qwen2.5": "#1f77b4", "qwen3": "#ff7f0e", "llama3.2": "#2ca02c",
       "gemma2": "#d62728", "phi3": "#9467bd", "mistral": "#8c564b",
       "deepseek-r1": "#e377c2"}


def load():
    out = []
    for fn in sorted(glob.glob(os.path.join(ROOT, "eval_results", "*.json"))):
        d = json.load(open(fn))
        if d.get("novel_pos_hits", -1) < 0:
            continue
        fam = d["model"].split(":")[0]
        out.append((d["model"], fam, d))
    return sorted(out, key=lambda x: x[0])


def main():
    rows = load()
    fig, axs = plt.subplots(2, 2, figsize=(13, 8))
    for ax, key, ttl in [
        (axs[0][0], "anom", "anomaly (%d)" % rows[0][2]["anom_t"]),
        (axs[0][1], "io", "io-next (%d)" % rows[0][2]["io_t"]),
        (axs[1][0], "cls", "classify (%d)" % rows[0][2]["cls_t"]),
        (axs[1][1], "novel_pos_hits", "novel blind-spot (10)"),
    ]:
        xs, ys, cs = [], [], []
        for i, (m, fam, d) in enumerate(rows):
            v = d[key]
            if key == "novel_pos_hits" and v < 0:
                continue
            xs.append(m)
            ys.append(v)
            cs.append(COL.get(fam, "#333"))
        ax.bar(xs, ys, color=cs)
        ax.set_title(ttl)
        ax.set_ylim(0, max(ys) * 1.15 if ys else 1)
        ax.tick_params(axis="x", rotation=45, labelsize=7)
        ax.grid(axis="y", alpha=0.3)
    fig.suptitle("B20: 15 local models x 100-sample m141 goldset (rules baseline: "
                 "anom 30/40 io 30/30 cls 16/30 novel 0/10; blue=qwen2.5 orange=qwen3 "
                 "green=llama red=gemma purple=phi brown=mistral pink=deepseek)", fontsize=9)
    fig.tight_layout()
    outp = os.path.join(r"D:\Dev\FujoOS-private\docs", "quality-curve-15.png")
    fig.savefig(outp, dpi=300)
    print("saved", outp)


if __name__ == "__main__":
    main()
