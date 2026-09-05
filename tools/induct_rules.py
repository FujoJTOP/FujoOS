#!/usr/bin/env python3
"""induct_rules.py — A4/B22: 判别式确定性规则归纳器 (v2)

v1 缺陷 (docs/98 A4): 跨样本公共子串按频率选 needle -> "ev "->1 过泛
(候选集内 fidelity 100%, 但命中一切 ev 前缀的正常事件)。
v2 (B22): 针判别归纳 —— 每个正样本选最长子串 needle, 且:
  (1) 不命中任何同 duty 负样本 (neg 命中 = 误判);
  (2) 命中其他正样本时其 value 必须一致 (否则换更短子串/整文本);
输出与 distill_rules.BAKED 同构 (供蒸馏合并); 模拟: 正样本 fidelity 100%
(断言) + 负样本误判 0 (断言)。
用法: python tools/induct_rules.py --in sdk/rulebook/train_cases_w23.json
      --neg sdk/rulebook/neg_samples_w34.json [--out sdk/rulebook/inducted.json]
"""
import argparse
import collections
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def word_needles(text, minlen=6, maxspan=3):
    """词边界内的连续词序列候选 (短到长: span=1 单词优先; 长度 >= minlen)。"""
    words = text.split(" ")
    n = len(words)
    for span in range(1, maxspan + 1):
        for i in range(n - span + 1):
            s = " ".join(words[i:i + span])
            if len(s) >= minlen:
                yield s


def induce(cases, negs, minlen=6):
    neg_by_duty = collections.defaultdict(list)
    for n in negs:
        neg_by_duty[n["duty"]].append(n["text"])

    # 按 value 分组正样本; 同一 (duty, needle) 只允许同一 value
    by_key = collections.defaultdict(list)
    for c in cases:
        by_key[(c["duty"], c["value"])].append(c)
    by_duty = collections.defaultdict(list)
    for c in cases:
        by_duty[c["duty"]].append(c)

    rules = []
    used_needles = collections.defaultdict(set)

    def bad(needle, duty, value):
        # (1) 负样本命中 = 误判
        for t in neg_by_duty[duty]:
            if needle in t:
                return True
        # (2) 其他正样本 value 不一致 = 误判
        for c in by_duty[duty]:
            if needle in c["text"] and c["value"] != value:
                return True
        return False

    for c in cases:
        if any(c["needle"] in c["text"] for c in []):
            pass
        chosen = None
        for s in word_needles(c["text"], minlen):
            if bad(s, c["duty"], c["value"]):
                continue
            chosen = s
            break
        if chosen is None:
            chosen = c["text"]
        if chosen in used_needles[c["duty"]]:
            continue
        used_needles[c["duty"]].add(chosen)
        # value 一致的该 needle 命中组 (取众数期望)
        hits = [x for x in by_key[(c["duty"], c["value"])] if chosen in x["text"]]
        v, a0, a1 = (c["value"], c["a0"], c["a1"])
        if hits:
            mc = collections.Counter((x["value"], x["a0"], x["a1"]) for x in hits)
            (v, a0, a1), _n = mc.most_common(1)[0]
        rules.append({"duty": c["duty"], "text": chosen, "needle": chosen,
                      "value": v, "a0": a0, "a1": a1,
                      "conf": max(x["conf"] for x in hits) if hits else c["conf"],
                      "param": 0})
    return rules


def simulate(rules, cases):
    covered = kept = 0
    for c in cases:
        hit = None
        for r in rules:
            if r["duty"] == c["duty"] and r["needle"] in c["text"]:
                hit = r
                break
        if hit is not None:
            covered += 1
            if hit["value"] == c["value"] and hit["a0"] == c["a0"] and hit["a1"] == c["a1"]:
                kept += 1
    return 100.0 * covered / len(cases), 100.0 * kept / len(cases)


def simulate_neg(rules, negs):
    """命中任一负样本 = 误判; 返回误判数。"""
    bad = 0
    for n in negs:
        for r in rules:
            if r["duty"] == n["duty"] and r["needle"] in n["text"]:
                bad += 1
                break
    return bad


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--in", dest="inp", default=os.path.join(ROOT, "sdk", "rulebook", "train_cases_w23.json"))
    ap.add_argument("--neg", default=os.path.join(ROOT, "sdk", "rulebook", "neg_samples_w34.json"))
    ap.add_argument("--minlen", type=int, default=6)
    ap.add_argument("--out", default=os.path.join(ROOT, "sdk", "rulebook", "inducted.json"))
    a = ap.parse_args()
    cases = json.load(open(a.inp))["cases"]
    negs = json.load(open(a.neg))["cases"]
    rules = induce(cases, negs, a.minlen)
    cov, keep = simulate(rules, cases)
    bad = simulate_neg(rules, negs)
    print(f"[induct-v2] rules={len(rules)} coverage={cov:.0f}% fidelity={keep:.0f}% "
          f"neg-mispredictions={bad}/{len(negs)}", flush=True)
    for r in rules:
        print(f"[induct-v2]   duty={r['duty']} needle={r['needle']!r} -> {r['value']}", flush=True)
    json.dump(rules, open(a.out, "w"), indent=1)
    print(f"[induct-v2] wrote {a.out}", flush=True)
    ok = (keep == 100.0 and bad == 0)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
