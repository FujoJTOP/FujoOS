#!/usr/bin/env python3
"""tau_derivation.py — trust-adaptive threshold derivation (zcode #6, review v2).

Data: B20 15 models x 100 samples (private eval_results/*.json; Q_total =
(anom+io+cls)/100; exogenous G/B marker = novel blind-spot coverage >= 1).

Loss (explicit, review point 3):
  L(tau; pi) = (1-pi) * Cs * P(Q < tau | G) + pi * Cw * P(Q > tau | B)
  estimates: P(Q<tau|G) = #{g in G: Q_g < tau} / n_G ; similarly for B.
Units: tau dimensionless (hit rate), Cs/Cw in same (per-decision) units.

Outputs: (1) in-sample tau* for Cs/Cw in {1/3, 1, 3}; (2) leave-one-model-out
tau* set (empirical pi and policy pi=1/3); (3) tau(pi) curve + minimax;
(4) k* Pareto alpha^k vs latency cost (k* = 2 region).
"""
import glob
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA = [r"D:\Dev\FujoOS-private\eval_results"]
sys.path.insert(0, os.path.join(ROOT, "tools"))


def load():
    out = {}
    for base in DATA:
        for fn in sorted(glob.glob(os.path.join(base, "*.json"))):
            d = json.load(open(fn))
            if d.get("novel_pos_hits", -1) < 0:
                continue
            m = d["model"]
            if m in out:
                continue
            out[m] = {"q": (d["anom"] + d["io"] + d["cls"]) / 100.0,
                      "novel": d["novel_pos_hits"]}
    # G/B marker: novel blind-spot coverage (S2, exogenous to Q)
    g = [v["q"] for v in out.values() if v["novel"] >= 1]
    b = [v["q"] for v in out.values() if v["novel"] == 0]
    return g, b


def loss(tau, g, b, pi, cs, cw):
    if not g or not b:
        return None
    p_gt = sum(1 for x in b if x > tau) / len(b)
    p_lt = sum(1 for x in g if x < tau) / len(g)
    return (1 - pi) * cs * p_lt + pi * cw * p_gt


def tau_star(g, b, pi, cs, cw):
    best = min(((loss(t / 100, g, b, pi, cs, cw), t / 100) for t in range(1, 100)),
               key=lambda z: z[0])
    return best[1], best[0]


def main():
    g, b = load()
    print(f"data: {len(g)} G (Q {min(g):.2f}-{max(g):.2f}), "
          f"{len(b)} B (Q {min(b):.2f}-{max(b):.2f})")

    # (1) in-sample, empirical prior, Cs/Cw sweep
    pi0 = len(b) / (len(g) + len(b))
    print(f"\n(1) in-sample (pi_hat={pi0:.2f}):")
    for cw, cs in [(3, 1), (1, 1), (1, 3)]:
        t, l = tau_star(g, b, pi0, cs, cw)
        l70 = loss(0.70, g, b, pi0, cs, cw)
        l30 = loss(0.30, g, b, pi0, cs, cw)
        print(f"  Cs/Cw={cs/cw:.2f}: tau*={t:.2f} L={l:.4f} | "
              f"L(tau=0.70)={l70:.4f} ({l70/l if l else float('inf'):.1f}x) | "
              f"L(tau=0.30)={l30:.4f}")

    # (2) leave-one-model-out (empirical pi per fold)
    print("\n(2) LOO tau* (empirical pi per fold, Cs=Cw=1):")
    folds = []
    for gi, q in enumerate(g):
        gg = [x for x in g if x != q]
        folds.append((f"G{gi} (Q={q:.2f})", tau_star(gg, b, len(b) / (len(gg) + len(b)), 1, 1)[0]))
    for bi, q in enumerate(b):
        bb = [x for x in b if x != q]
        folds.append((f"B{bi} (Q={q:.2f})", tau_star(g, bb, len(bb) / (len(g) + len(bb)), 1, 1)[0]))
    stars = [s for _, s in folds]
    for label, s in folds:
        print(f"  remove {label:20s} -> tau*={s:.2f}")
    print(f"  tau*_LOO set: {sorted(set(stars))}; "
          f"main cluster {min(stars):.2f}-{max(s for s in stars if s < 0.4):.2f} "
          f"({sum(1 for s in stars if s < 0.4)}/{len(stars)} folds), "
          f"outlier 0.46 ({sum(1 for s in stars if s >= 0.4)} fold)")
    # policy-fixed pi=1/3 LOO (deployment view)
    stars2 = [tau_star([x for x in g if x != q], b, 1 / 3, 1, 1)[0] for q in g]
    stars2 += [tau_star(g, [x for x in b if x != q], 1 / 3, 1, 1)[0] for q in b]
    print(f"  tau*_LOO (policy pi=1/3): {sorted(set(stars2))} "
          f"({sum(1 for s in stars2 if s < 0.4)}/{len(stars2)} folds at cluster)")

    # (3) tau(pi) curve + minimax
    print("\n(3) tau(pi) (Cs=Cw=1):")
    pts = []
    for pi10 in range(1, 10):
        t, l = tau_star(g, b, pi10 / 10, 1, 1)
        pts.append((pi10 / 10, t))
        print(f"  pi={pi10/10:.1f}: tau*={t:.2f}")
    mm = min(((max(loss(t / 100, g, b, pi / 100, 1, 1)
                   for pi in range(5, 96)), t / 100) for t in range(1, 100)),
             key=lambda z: z[0])
    print(f"  minimax tau* = {mm[1]:.2f} (worst-case L={mm[0]:.4f})")

    # (4) k Pareto: alpha(tau) -> alpha^k vs latency cost, both candidate taus
    print("\n(4) k* (alpha = P(B single 64-pt window >= tau), normal approx):")
    import math
    import statistics
    mu = statistics.mean(b)
    sd = statistics.pstdev(b)
    sw = sd / ((64 / 100) ** 0.5)
    for tau in (0.35, 0.46):
        alpha = 0.5 * (1 - math.erf((tau - mu) / (sw * 2 ** 0.5)))
        print(f"  tau_hi={tau}: B mu={mu:.3f} sd={sd:.3f} sd_w={sw:.3f} -> "
              f"alpha~{alpha:.4f}")
        for k in (1, 2, 3, 4):
            print(f"    k={k}: P(consecutive high windows) ~ {alpha**k:.6f}")
        for L10 in (-3, -2, -1, 0):
            L = 10 ** L10
            ks = [alpha ** k + L * k for k in (1, 2, 3, 4)]
            print(f"    L=1e{L10}: k*={ks.index(min(ks)) + 1}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
