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
PARAMS = {"qwen2.5:0.5b": 0.5, "qwen2.5:1.5b": 1.5, "qwen2.5:3b": 3,
          "qwen2.5:7b": 7, "qwen3:0.6b": 0.6, "qwen3:1.7b": 1.7,
          "qwen3:4b": 4, "qwen3:8b": 8, "llama3.2:1b": 1, "llama3.2:3b": 3,
          "gemma2:2b": 2, "gemma2:9b": 9, "phi3:mini": 3.8,
          "mistral:7b": 7, "deepseek-r1:1.5b": 1.5}


def load():
    out = []
    for base in (os.path.join(ROOT, "eval_results"),
                 r"D:\Dev\FujoOS-private\eval_results"):
        for fn in sorted(glob.glob(os.path.join(base, "*.json"))):
            d = json.load(open(fn))
            if d.get("novel_pos_hits", -1) < 0:
                continue
            fam = d["model"].split(":")[0]
            out.append((d["model"], fam, d))
    return sorted(out, key=lambda x: (FAM.index(x[1]), PARAMS.get(x[0], 0)))


def main():
    rows = load()
    fig, axs = plt.subplots(2, 3, figsize=(15, 8))
    for ax, key, ttl in [
        (axs[0][0], "anom", "anomaly (%d)" % rows[0][2]["anom_t"]),
        (axs[0][1], "io", "io-next (%d)" % rows[0][2]["io_t"]),
        (axs[0][2], "cls", "classify (%d)" % rows[0][2]["cls_t"]),
        (axs[1][0], "novel_pos_hits", "novel blind-spot (10)"),
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
        # zero-value bars stay visible (0/30 = real, not missing)
        for i, v in enumerate(ys):
            if v == 0:
                ax.annotate("0", (xs[i], 0), textcoords="offset points",
                            xytext=(0, 2), fontsize=6, ha="center", color="#777")
    # scale vs blind-spot coverage: the corrected B3 claim (non-monotone)
    ax = axs[1][1]
    for m, fam, d in rows:
        ax.scatter(PARAMS[m], d["novel_pos_hits"], color=COL.get(fam, "#333"),
                   s=60, label=fam if d["model"] == rows[0][0] else None)
    for m, fam, d in rows:
        ax.annotate(m.replace(":", " "), (PARAMS[m], d["novel_pos_hits"]),
                    textcoords="offset points", xytext=(4, 4), fontsize=5.5,
                    zorder=6,
                    bbox=dict(facecolor="white", alpha=0.8, edgecolor="none", pad=0.5))
    ax.set_xscale("log")
    ax.set_xlabel("params (B, log)")
    ax.set_ylabel("novel blind-spot hits /10")
    ax.set_title("coverage is NOT size-monotone (0.6B 10/10 vs 4B 0/10)")
    ax.grid(alpha=0.3)
    # LOO result summary panel (one text call per line: \n multi-line text has
    # unreliable linespacing on some backends -> overlap)
    ax = axs[1][2]
    ax.axis("off")
    for s, y in [
        ("Leave-one-model-out (15 removals):", 0.96),
        ("C1 blind-spot coverage: no flip", 0.78),
        ("C2 io rule-ownership: no flip", 0.60),
        ("C3 worst model: qwen2.5:0.5b (32)", 0.42),
        ("C4 orthogonality: FLIPS when", 0.24),
        ("   llama3.2:3b removed (dual-best)", 0.08),
    ]:
        ax.text(0, y, s, fontsize=9, family="monospace")
    fig.suptitle("B20: 15 local models x 100-sample m141 goldset (rules baseline: "
                 "anom 30/40 io 30/30 cls 16/30 novel 0/10; blue=qwen2.5 orange=qwen3 "
                 "green=llama red=gemma purple=phi brown=mistral pink=deepseek)", fontsize=9)
    fig.tight_layout()
    outp = os.path.join(r"D:\Dev\FujoOS-private\docs", "quality-curve-15.png")
    fig.savefig(outp, dpi=300, bbox_inches="tight")
    print("saved", outp)


if __name__ == "__main__":
    main()
