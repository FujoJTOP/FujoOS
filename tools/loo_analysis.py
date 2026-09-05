#!/usr/bin/env python3
"""loo_analysis.py — B20: leave-one-model-out robustness of B3 conclusions.

Reads eval_results/*.json (15 models x 100 samples), recomputes the
conclusion set on the full population and after removing each one model:
  C1 novel blind-spot coverage (novel_pos_hits == all) has a scale threshold;
  C2 io: rules baseline > every model;
  C3 smallest models unusable (0.5b-style all-wrong);
  C4 blind-spot coverage is orthogonal to total anom accuracy.
Prints a table of which removal flips which conclusion.
"""
import glob
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RES = os.path.join(ROOT, "eval_results")


def load():
    out = []
    for fn in sorted(glob.glob(os.path.join(RES, "*.json"))):
        d = json.load(open(fn))
        if d.get("novel_pos_hits", -1) < 0:
            continue
        out.append(d)
    return out


def con(dat):
    """Conclusion set on a population. dat: list of per-model dicts."""
    c = {}
    # C1: blind-spot coverage; population hit = max across models
    c["c1_best_coverage"] = max(d["novel_pos_hits"] for d in dat)
    c["c1_coverage_full"] = any(d["novel_pos_hits"] == 10 for d in dat)
    # C2: io rules-owned: rules (9/10 at n=10 -> recompute baseline separately;
    # here: deterministic baseline is 0 for novel cycles -> compare models to 30)
    c["c2_io_best"] = max(d["io"] for d in dat)
    c["c2_io_all_below"] = all(d["io"] < 30 for d in dat)  # rules=30/30
    # C3: smallest unusable (lowest family model)
    c["c3_min_total"] = min(d["anom"] + d["io"] + d["cls"] for d in dat)
    # C4: orthogonality: best coverage model is not the best anom accuracy
    best_cov = [d for d in dat if d["novel_pos_hits"] == c["c1_best_coverage"]]
    best_anom = max(d["anom"] for d in dat)
    orth = [d for d in best_cov if d["anom"] == best_anom]
    c["c4_orthogonal"] = not orth
    return c


def main():
    dat = load()
    if len(dat) < 3:
        print("need >=3 model jsons", file=sys.stderr)
        return 1
    full = con(dat)
    print(f"full population (n={len(dat)}): {json.dumps(full)}")
    print(f"{'removed':20s} {'c1':8s} {'c2':8s} {'c3':8s} {'c4':8s} flips")
    for d in dat:
        sub = [x for x in dat if x["model"] != d["model"]]
        s = con(sub)
        flips = [k for k in ("c1_coverage_full", "c2_io_all_below", "c4_orthogonal")
                 if s[k] != full[k]]
        if s["c3_min_total"] != full["c3_min_total"]:
            flips.append(f"c3({full['c3_min_total']}->{s['c3_min_total']})")
        print(f"{d['model']:20s} {str(s['c1_coverage_full']):8s} {str(s['c2_io_all_below']):8s} "
              f"{s['c3_min_total']:<8d} {str(s['c4_orthogonal']):8s} {','.join(flips)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
