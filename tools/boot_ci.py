#!/usr/bin/env python3
"""boot_ci.py — B20 statistical support (bootstrap + Wilson CI).

口径对齐 tau_derivation.py (zcode): G = novel-pos >= 1 (10), B = novel == 0 (5),
loss L(tau;pi) = (1-pi)*Cs*P(Q<tau|G) + pi*Cw*P(Q>tau|B); Q_total = (anom+io+cls)/100.
Outputs:
  (1) per-model Wilson 95% CI for Q_total and novel coverage;
  (2) model-level bootstrap (B=2000, resample 15 with replacement):
      tau* distribution (Cs/Cw=3, pi=1/3), P(tau* in the two-tier set {0.35,0.46}),
      C1 hold-rate (some G with full novel coverage), C2 hold-rate
      (rules io 30/30 > every model);
  (3) readable summary for docs/108 (private) + paper §8.1 reference.
"""
import glob
import json
import math
import os
import random

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CS, CW = 1.0, 3.0
PI = 1.0 / 3.0
B = 2000


def load():
    out = {}
    for base in (os.path.join(ROOT, "eval_results"),
                 r"D:\Dev\FujoOS-private\eval_results"):
        for fn in sorted(glob.glob(os.path.join(base, "*.json"))):
            d = json.load(open(fn))
            if d.get("novel_pos_hits", -1) < 0:
                continue
            tot = d["anom_t"] + d["io_t"] + d["cls_t"]
            out[d["model"]] = {
                "q": (d["anom"] + d["io"] + d["cls"]) / tot,
                "novel": d["novel_pos_hits"],
                "nom": d["anom"], "not": d["anom_t"], "io": d["io"], "iot": d["io_t"],
                "cls": d["cls"], "clst": d["cls_t"],
            }
    return out


def wilson(k, n, z=1.96):
    if n == 0:
        return (0.0, 0.0)
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return max(0.0, c - half), min(1.0, c + half)


def loss(tau, g, b, pi=PI, cs=CS, cw=CW):
    p_gt = sum(1 for x in b if x > tau) / len(b)
    p_lt = sum(1 for x in g if x < tau) / len(g)
    return (1 - pi) * cs * p_lt + pi * cw * p_gt


def tau_star(g, b, pi=PI, cs=CS, cw=CW):
    return min(((loss(t / 100, g, b, pi, cs, cw), t / 100) for t in range(1, 100)),
               key=lambda x: x[0])[1]


def main():
    dat = load()
    print(f"n_models={len(dat)}")
    g = [v["q"] for v in dat.values() if v["novel"] >= 1]
    b = [v["q"] for v in dat.values() if v["novel"] == 0]
    print(f"G={len(g)} (novel>=1), B={len(b)} (novel==0)")
    print(f"G range [{min(g):.3f}, {max(g):.3f}]  B range [{min(b):.3f}, {max(b):.3f}]")
    # (1) per-model Wilson
    print("\n== per-model Wilson 95% CI ==")
    for m in sorted(dat, key=lambda k: -dat[k]["q"]):
        v = dat[m]
        t = v["not"] + v["iot"] + v["clst"]
        k = int(round(v["q"] * t))
        lo, hi = wilson(k, t)
        sl, sh = wilson(v["novel"], 10)
        print(f"{m:20s} Q={v['q']:.3f} [{lo:.3f},{hi:.3f}]  novel={v['novel']}/10 [{sl:.2f},{sh:.2f}]")
    # (2) model-level bootstrap
    models = list(dat.keys())
    taustar, c1_hold, c2_hold = [], 0, 0
    for _ in range(B):
        samp = [dat[random.choice(models)] for _ in models]
        sg = [x["q"] for x in samp if x["novel"] >= 1]
        sb = [x["q"] for x in samp if x["novel"] == 0]
        if not sg or not sb:
            continue
        taustar.append(tau_star(sg, sb))
        if max((x["novel"] for x in samp), default=-1) >= 1:
            c1_hold += 1
        if max((x["io"] for x in samp), default=-1) < 30:
            c2_hold += 1
    taustar.sort()
    n = len(taustar)
    p5 = taustar[int(0.05 * n)]
    p50 = taustar[int(0.5 * n)]
    p95 = taustar[int(0.95 * n)]
    in_two = sum(1 for t in taustar if abs(t - 0.35) < 1e-9 or abs(t - 0.46) < 1e-9) / n
    print(f"\n== model-level bootstrap (B={B}) ==")
    print(f"tau* distribution: p5={p5:.3f} p50={p50:.3f} p95={p95:.3f}")
    print(f"P(tau* in {{0.35,0.46}}) = {in_two:.3f}")
    print(f"C1 hold (some G with full novel coverage): {c1_hold / B:.3f}")
    print(f"C2 hold (rules io > every model): {c2_hold / B:.3f}")


if __name__ == "__main__":
    main()
