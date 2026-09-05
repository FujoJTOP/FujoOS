#!/usr/bin/env python3
"""plot_quality_curve.py — B3: 三模型质量曲线 (m141 goldset n=36)

数据源: docs/101-b3-quality-curve.md (同集同链路实测)。
输出: docs/quality-curve.png (300dpi 论文级, 2x2 子图 + 规模-盲区覆盖主折线)。
用法: uv run --with matplotlib python tools/plot_quality_curve.py
"""
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "docs", "quality-curve.png")

# ---- 实测数据 (docs/101) ----
MODELS = ["qwen2.5:0.5b", "qwen2.5:7b", "qwen3:8b"]
SCALE = [0.5, 7.0, 8.0]  # 参数规模 (B)
DUTIES = [
    ("anomaly sentinel", [9, 16, 9], 12, 16, "anom", "#1f77b4"),
    ("io prediction",     [0, 3, 4], 9, 10, "io", "#ff7f0e"),
    ("intent classify",   [4, 8, 8], 4, 10, "cls", "#2ca02c"),
    ("blind-spot coverage (novel)", [0, 4, 4], 0, 4, "novel", "#d62728"),
]

fig, axes = plt.subplots(2, 2, figsize=(10, 7.5))

for ax, (name, vals, base, denom, key, color) in zip(axes.flat, DUTIES):
    y = [100.0 * v / denom for v in vals]
    ax.plot(SCALE, y, "o-", color=color, lw=2, ms=7, zorder=3, label="model")
    ax.axhline(100.0 * base / denom, color="grey", ls="--", lw=1.4,
               label=f"deterministic rules baseline ({base}/{denom})")
    ax.set_xticks(SCALE)
    ax.set_xticklabels(["0.5B", "7B", "8B"], fontsize=10)
    ax.set_xlim(0, 9)
    ax.set_ylim(-5, 108)
    ax.set_ylabel("accuracy (%)", fontsize=9)
    ax.set_title(f"{name} ({denom} samples)", fontsize=11)
    ax.grid(alpha=0.3, ls=":")
    for xi, yi, v in zip(SCALE, y, vals):
        ax.annotate(f"{v}/{denom}", (xi, yi), textcoords="offset points",
                    xytext=(0, 8), ha="center", fontsize=8, color=color)
    ax.legend(fontsize=8, loc="lower right" if key != "io" else "center right")

fig.suptitle("Model scale vs. quality — FujoOS AI organ, m141 goldset n=36 "
             "(same set & link, three engines)", fontsize=13)
fig.tight_layout(rect=(0, 0, 1, 0.96))
fig.savefig(OUT, dpi=300)
print(f"wrote {OUT}")
