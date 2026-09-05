#!/usr/bin/env python3
"""make_paper_figs.py — paper figures for FUAI (private outputs).

Figures (all saved to D:\\Dev\\FujoOS-private\\docs):
  fig-arch.png        FUAI architecture (organs, channel, arbiter, domain)
  fig-regression.png  regression growth (9->40) + cumulative milestones
  fig-gsn.png         GSN-style assurance case (S1 x S2 x S3, measurement envelope)
  fig-latency.png     per-model inference latency (L1/L2/L3 bands)
Also extracts per-model latency medians -> latency-models.json (first run).
Usage: uv run --with matplotlib python tools/make_paper_figs.py
"""
import glob
import json
import os
import re
import statistics
import tempfile

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.patches import FancyArrowPatch, FancyBboxPatch

PRIV = r"D:\Dev\FujoOS-private\docs"
LAT = {
    "qwen2.5:0.5b": 0.5, "qwen2.5:1.5b": 1.5, "qwen2.5:3b": 3, "qwen2.5:7b": 7,
    "qwen3:0.6b": 0.6, "qwen3:1.7b": 1.7, "qwen3:4b": 4, "qwen3:8b": 8,
    "llama3.2:1b": 1, "llama3.2:3b": 3, "gemma2:2b": 2, "gemma2:9b": 9,
    "phi3:mini": 3.8, "mistral:7b": 7, "deepseek-r1:1.5b": 1.5,
}


def latencies():
    """Extract (model -> median seconds) from ai-verify-*/server.log; cache json."""
    jp = os.path.join(PRIV, "latency-models.json")
    if os.path.exists(jp):
        return json.load(open(jp))
    med = {}
    pat = re.compile(r"\((qwen[^)]*|llama3\.2[^)]*|gemma[^)]*|"
                     r"phi3[^)]*|mistral[^)]*|deepseek[^)]*)\) in ([0-9.]+)s")
    for d in glob.glob(os.path.join(tempfile.gettempdir(), "ai-verify-*")):
        sl = os.path.join(d, "server.log")
        if not os.path.exists(sl):
            continue
        for l in open(sl, errors="replace"):
            m = pat.search(l)
            if m:
                med.setdefault(m.group(1), []).append(float(m.group(2)))
    med = {k: statistics.median(v) for k, v in med.items()
           if len(v) >= 20 and "unparsed" not in k and
           k.split(":")[0] in
           ("qwen2.5", "qwen3", "llama3.2", "gemma2", "phi3", "mistral", "deepseek-r1")}
    json.dump(med, open(jp, "w"), indent=1)
    return med


def box(ax, x, y, w, h, text, fc="#eef3f8", ec="#333", fs=9):
    ax.add_patch(FancyBboxPatch((x, y), w, h, boxstyle="round,pad=0.02",
                                fc=fc, ec=ec, lw=1.2))
    ax.text(x + w / 2, y + h / 2, text, ha="center", va="center", fontsize=fs)


def arrow(ax, x1, y1, x2, y2, label="", fs=7):
    ax.add_patch(FancyArrowPatch((x1, y1), (x2, y2), arrowstyle="-|>",
                                 mutation_scale=10, color="#555", lw=1.2))
    if label:
        ax.text((x1 + x2) / 2, (y1 + y2) / 2 + 0.08, label, ha="center",
                fontsize=fs, color="#555",
                bbox=dict(facecolor="white", alpha=0.9, edgecolor="none", pad=1.5))


def fig_arch():
    fig, ax = plt.subplots(figsize=(12, 6.6))
    ax.set_xlim(0, 12)
    ax.set_ylim(0, 6.6)
    ax.axis("off")
    # host side
    box(ax, 0.2, 4.4, 3.0, 1.5, "Host (outside kernel)\nOllama model 0.5B-9B\n(15 local, B20)\nprotocol server (COM2 + shm)", fc="#fdf3e3")
    # kernel organs
    box(ax, 4.2, 5.2, 3.4, 0.7, "FUAI organ: 5 duties", fc="#e3efdc")
    names = ["sentinel", "planner", "io-predict", "nlc", "env"]
    for i, n in enumerate(names):
        box(ax, 4.2 + i * 0.7, 4.5, 0.64, 0.55, n, fc="#e3efdc", fs=7)
    # channel + event ring
    box(ax, 4.2, 3.5, 3.4, 0.7, "shared-memory channel + event ring (eyes)\n0x8304 anom / 0x5101 intent / 0x8306 io / 0x8307 nlc / 0x8308 env", fc="#dce9f0", fs=7)
    # engine selector
    box(ax, 4.2, 2.6, 3.4, 0.6, "engine select 0x830F: model | rules | auto\n(absence = first-class state)", fc="#dce9f0", fs=7.5)
    # arbiter + domain
    box(ax, 8.4, 3.9, 3.2, 0.9, "rule arbiter (final)\nFJRU bytecode + fallback", fc="#f5e3e3")
    box(ax, 8.4, 2.6, 3.2, 0.9, "cap_exec + revocable domain\nblast-radius bound (M116)", fc="#f5e3e3")
    # quality ledger / dom_admit
    box(ax, 8.4, 1.3, 3.2, 0.9, "quality ledger 0x8314 -> dom_admit 0x8313\n(domain width = f(quality); W32)", fc="#ece3f5")
    # audit
    box(ax, 4.2, 1.3, 3.4, 0.8, "audit ring (all duties carry self-labeled\nverification; act_verify / ev_digest)", fc="#efe6da")
    arrow(ax, 3.2, 5.1, 4.2, 5.6, "FJAI:REQ/RSP")
    arrow(ax, 5.9, 4.5, 5.9, 4.2)
    arrow(ax, 5.9, 3.5, 5.9, 3.2)
    arrow(ax, 7.6, 3.3, 8.4, 4.1)
    arrow(ax, 7.6, 2.9, 8.4, 2.9)
    arrow(ax, 10.0, 2.6, 10.0, 2.2)
    arrow(ax, 7.6, 1.8, 8.4, 1.75, "qual_feed")
    ax.set_title("FUAI: model as a system organ (proposes) - kernel disposes\n"
                 "audit at every exchange; rules final arbiter; absence degrades to rules")
    fig.tight_layout()
    fig.savefig(os.path.join(PRIV, "fig-arch.png"), dpi=300, bbox_inches="tight")
    plt.close(fig)


def fig_regression():
    waves = ["W20", "W21", "W22", "W23", "W27", "W33/B20"]
    reg = [29, 30, 33, 34, 37, 40]
    ms = [138, 140, 142, 143, 147, 150]
    fig, ax = plt.subplots(figsize=(8, 4.4))
    ax.plot(waves, reg, "o-", color="#1f77b4", label="regression cases (PASS)")
    ax.set_ylabel("regression cases", color="#1f77b4")
    ax.set_ylim(20, 45)
    ax2 = ax.twinx()
    ax2.plot(waves, ms, "s--", color="#d62728", label="milestone count")
    ax2.set_ylabel("milestones", color="#d62728")
    ax2.set_ylim(120, 160)
    for x, y in zip(waves, reg):
        ax.annotate(str(y), (x, y), textcoords="offset points", xytext=(0, 12),
                    fontsize=8, zorder=6,
                    bbox=dict(facecolor="white", alpha=0.85, edgecolor="none", pad=0.5))
    ax.grid(alpha=0.3)
    ax.set_title("FujoOS verification growth: 29->40 regressions alongside milestones")
    fig.tight_layout()
    fig.savefig(os.path.join(PRIV, "fig-regression.png"), dpi=300, bbox_inches="tight")
    plt.close(fig)


def gnode(ax, x, y, w, h, kind, text, fs=7.5):
    fc = {"Goal": "#dce9f0", "Strategy": "#efe6da", "Claim": "#e3efdc",
          "Evidence": "#fdf3e3", "Context": "#f0f0f0", "Assumption": "#f5e3e3"}[kind]
    box(ax, x, y, w, h, text, fc=fc, fs=fs)
    ax.text(x + 0.06, y + h - 0.09, kind, fontsize=6.5, va="top", color="#555", fontweight="bold")


def fig_gsn():
    fig, ax = plt.subplots(figsize=(12, 6.6))
    ax.set_xlim(0, 12)
    ax.set_ylim(0, 6.6)
    ax.axis("off")
    gnode(ax, 4.2, 5.7, 3.6, 0.75, "Goal", "AI organ is safe:\nS1 mechanism AND S2 entrustment AND S3 policy")
    gnode(ax, 4.4, 4.55, 3.2, 0.65, "Strategy", "three-modal orthogonal correctness;\nmeasurement-parameterized envelope")
    gnode(ax, 0.2, 3.0, 3.3, 1.1, "Claim", "S1: for all traces,\nactualActions subset domain cap grant\n(kernel-checked axioms A1-A4)")
    gnode(ax, 4.4, 3.0, 3.3, 1.1, "Claim", "S2: entrustment = measured\nblind-spot coverage, domain width\nf(quality ledger, W32)")
    gnode(ax, 8.6, 3.0, 3.3, 1.1, "Claim", "S3: declared policy (rules final\narbiter; thresholds tau_high/tau_low\ncfg7/cfg8)")
    gnode(ax, 0.2, 1.6, 3.3, 0.9, "Evidence", "M119 axiom prober + M116 blast\nradius + m141 rules novel-pos 0/10\n+ regression 40/40 (3 exec modes)")
    gnode(ax, 4.4, 1.6, 3.3, 0.9, "Evidence", "B20: 15 models x 100 samples,\nleft-one-out: C1/C2 no flip;\nW32 m149 trust-adaptive demo")
    gnode(ax, 8.6, 1.6, 3.3, 0.9, "Evidence", "rule book + audit ring;\nW33 anti-abuse A7-2\n(2-window widen confirm)")
    gnode(ax, 3.7, 0.15, 5.0, 0.75, "Context", "claim parameterized by measurement (not model law):\nblind-spot coverage is family/instruction-following, NOT\nsize-monotone; leave-one-model-out check required (B20)")
    arrow(ax, 6.0, 5.7, 6.0, 5.2)
    arrow(ax, 5.2, 4.55, 3.2, 4.1, "")
    arrow(ax, 6.0, 4.55, 6.0, 4.1)
    arrow(ax, 6.9, 4.55, 9.4, 4.1)
    arrow(ax, 1.8, 3.0, 1.8, 2.5)
    arrow(ax, 6.0, 3.0, 6.0, 2.5)
    arrow(ax, 10.2, 3.0, 10.2, 2.5)
    ax.set_title("GSN-style assurance case: S1 x S2 x S3 with measurement-parameterized envelope (B20 revision)")
    fig.tight_layout()
    fig.savefig(os.path.join(PRIV, "fig-gsn.png"), dpi=300, bbox_inches="tight")
    plt.close(fig)


def fig_latency():
    med = latencies()
    rows = sorted(med.items(), key=lambda kv: LAT.get(kv[0], 9))
    fig, ax = plt.subplots(figsize=(9, 4.6))
    xs = [m for m, _ in rows]
    ys = [v for _, v in rows]
    ax.bar(xs, ys, color="#8c564b")
    ax.set_yscale("log")
    ax.set_ylabel("median inference time (s, log)")
    ax.tick_params(axis="x", rotation=60, labelsize=7)
    for x, y in zip(xs, ys):
        ax.annotate(f"{y:.2f}s", (x, y), textcoords="offset points",
                    xytext=(0, 3), fontsize=6.5, rotation=90)
    ax.axhline(0.001, color="#2ca02c", ls="--", lw=1)
    ax.text(0.015, 0.003, "L1 realtime band (us-ms)", ha="left", va="bottom",
            fontsize=7, color="#2ca02c", zorder=5,
            bbox=dict(facecolor="white", alpha=0.85, edgecolor="none", pad=1))
    ax.axhline(4.0, color="#d62728", ls="--", lw=1)
    ax.text(0.015, 9.0, "L3 staleness TTL hardening (4s budget)", ha="left", va="bottom",
            fontsize=7, color="#d62728", zorder=5,
            bbox=dict(facecolor="white", alpha=0.85, edgecolor="none", pad=1))
    ax.set_title("A5 latency: per-model median inference (L1 realtime / L2 measured\n"
                 "0.06-14.4s / L3 TTL 4s -> availability = latency<=budget AND TTL>=p95)")
    ax.grid(axis="y", alpha=0.3)
    fig.tight_layout()
    fig.savefig(os.path.join(PRIV, "fig-latency.png"), dpi=300, bbox_inches="tight")
    plt.close(fig)


if __name__ == "__main__":
    fig_arch()
    fig_regression()
    fig_gsn()
    fig_latency()
    print("saved 4 figures to", PRIV)
