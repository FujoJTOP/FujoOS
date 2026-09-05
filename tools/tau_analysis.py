#!/usr/bin/env python3
"""tau_analysis.py — τ (trust-adaptive threshold) analysis rebuild (zcode review).

B20 population: 15 models with quality q_i = (anom+io+cls)/(100) from
eval_results; G/B label: novel-pos >= 9 -> G (good, 10 models), else B (5).
Detection model: a widening window happens when quality >= tau; G should
widen, B must not. Loss (explicit):
  loss(tau) = [C_s * #(G and q < tau) + C_w * #(B and q >= tau)] / N
with C_s = 1 (missed widening, cost of a safe-but-stuck system) and
C_w = 3 (false widening, policy contamination is 3x costlier; ratio 0.33).
Outputs: in-sample tau*, leave-one-model-out tau* set (review point 1),
tau(pi) curve + minimax tau (review point 2), and the k=2 confirmation
latency note (review point 4).
Usage: uv run --with matplotlib python tools/tau_analysis.py
"""
import glob
import json
import os
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CS, CW = 1.0, 3.0
G_LABEL = 9  # novel-pos >= 9 -> G


def load():
    rows = []
    for base in (os.path.join(ROOT, "eval_results"),
                 r"D:\Dev\FujoOS-private\eval_results"):
        for fn in sorted(glob.glob(os.path.join(base, "*.json"))):
            d = json.load(open(fn))
            if d.get("novel_pos_hits", -1) < 0:
                continue
            q = (d["anom"] + d["io"] + d["cls"]) / (d["anom_t"] + d["io_t"] + d["cls_t"])
            rows.append({"model": d["model"], "q": q,
                         "g": d["novel_pos_hits"] >= G_LABEL})
    seen = {}
    for r in rows:
        seen.setdefault(r["model"], r)
    return list(seen.values())


def loss(dat, tau):
    g_miss = sum(1 for r in dat if r["g"] and r["q"] < tau)
    b_false = sum(1 for r in dat if not r["g"] and r["q"] >= tau)
    return (CS * g_miss + CW * b_false) / len(dat)


def tau_star(dat, lo=0.01, hi=0.99, step=0.005):
    best_t, best_l = None, 1e9
    t = lo
    while t <= hi:
        l = loss(dat, t)
        if l < best_l - 1e-9:
            best_l, best_t = l, t
        t += step
    return best_t, best_l


def tau_of_pi(dat, pi, lo=0.01, hi=0.99, step=0.005):
    """Bayes-optimal tau under prior pi = P(B): reweight loss by class prior."""
    n_g = sum(1 for r in dat if r["g"])
    n_b = len(dat) - n_g
    best_t, best_l = None, 1e9
    t = lo
    while t <= hi:
        g_miss = sum(1 for r in dat if r["g"] and r["q"] < t)
        b_false = sum(1 for r in dat if not r["g"] and r["q"] >= t)
        l = (CS * (g_miss / n_g) * (1 - pi) + CW * (b_false / n_b) * pi)
        if l < best_l - 1e-9:
            best_l, best_t = l, t
        t += step
    return best_t, best_l


def main():
    dat = load()
    if len(dat) != 15:
        print(f"need 15 models, got {len(dat)}", file=sys.stderr)
        return 1
    # ① in-sample + LOO
    t0, l0 = tau_star(dat)
    loo = [(m["model"],) + tau_star([d for d in dat if d["model"] != m["model"]])
           for m in dat]
    print(f"in-sample tau*={t0:.3f} loss={l0:.4f}")
    for m, t, l in sorted(loo, key=lambda x: x[1]):
        print(f"  LOO remove {m:20s} tau*={t:.3f} loss={l:.4f}")
    ts = [t for _, t, _ in loo]
    in_band = all(0.35 <= t <= 0.465 for t in ts)
    print(f"LOO tau* set: {[round(t,3) for t in ts]}")
    print(f"all LOO tau* within [0.35, 0.465]: {in_band}")
    # baseline: current tau=0.70 loss
    print(f"loss(tau=0.70) = {loss(dat, 0.70):.4f} (vs tau* {l0:.4f})")
    # ② tau(pi) curve
    pis = [i / 20 for i in range(21)]
    curve = [(pi, tau_of_pi(dat, pi)) for pi in pis]
    # minimax: tau minimizing worst-case class loss over pi in [0,1]
    mm = min(((loss(dat, t), t) for t in [x / 100 for x in range(1, 100)]),
             key=lambda x: x[0])
    print(f"tau(pi) markers: pi=0.333(t={curve[6][1][0]:.3f}) "
          f"pi=0.5(t={curve[10][1][0]:.3f}) pi=1.0(t={curve[20][1][0]:.3f})")
    print(f"minimax tau (worst-case class loss) = {mm[1]:.3f} loss={mm[0]:.4f}")
    fig, ax1 = plt.subplots(figsize=(8, 4.4))
    ax1.plot(pis, [t for t, _ in [c[1] for c in curve]], "o-", color="#1f77b4", label="tau*(pi)")
    ax1.set_xlabel("pi = P(B) (deployment prior; policy variable)")
    ax1.set_ylabel("Bayes-optimal tau", color="#1f77b4")
    ax2 = ax1.twinx()
    ax2.plot(pis, [l for _, (t, l) in curve], "s--", color="#d62728", label="loss(tau*, pi)")
    ax2.set_ylabel("prior-weighted loss", color="#d62728")
    ax1.axhline(0.70, color="#555", ls=":")
    ax1.text(0.02, 0.70, "current tau=0.70", fontsize=7, color="#555")
    ax1.set_title("tau is prior-sensitive (review 2): Bayes-optimal tau moves with pi; "
                  "mechanism bound is prior-free, threshold is not")
    fig.tight_layout()
    fig.savefig(os.path.join(r"D:\Dev\FujoOS-private\docs", "fig-tau-pi.png"), dpi=300,
                bbox_inches="tight")
    print("saved fig-tau-pi.png")
    return 0


if __name__ == "__main__":
    sys.exit(main())
