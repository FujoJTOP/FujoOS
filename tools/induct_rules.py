#!/usr/bin/env python3
"""induct_rules.py — A4 α: 确定性规则归纳器 (替代外部 7B 归纳步骤)

动机 (docs/98 A4): 蒸馏闭环的半自治缺口 = bake 需人工/外部 7B。S1/S2/S3 框架下
"规则来源不必须是 LLM" (S2 只要求盲区覆盖度被度量) —— 本脚本用确定性启发式
(候选样本的 token 频率子串 + 众数期望) 归纳 needle→value 规则, fidelity 门校验。

输入: 候选集 (与 distill_feed.py 输出同构: {duty, text, value, a0, a1, conf, param})
输出: 规则 JSON 列表 (与 distill_rules.BAKED 同构), 供 distill_rules.py 合并。
验证: 对候选集 simulate 100% (与 distill_rules.simulate 同语义)。
用法: python tools/induct_rules.py --in sdk/rulebook/train_cases_w23.json
      [--out sdk/rulebook/inducted.json]
"""
import argparse
import collections
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def freq_substrings(text: str, minlen: int = 3, maxlen: int = 12) -> collections.Counter:
    """非空格连续子串频率 (含空格分隔的 token 边界内)。"""
    c = collections.Counter()
    n = len(text)
    for i in range(n):
        if text[i] == ' ':
            continue
        for L in range(minlen, maxlen + 1):
            if i + L > n:
                break
            s = text[i:i + L]
            if ' ' in s:
                # 允许边界但 needle 需连续; 简单起见保留非空空白子串
                pass
            c[s] += 1
    return c


def induce(cases):
    rules = []
    by_duty = collections.defaultdict(list)
    for c in cases:
        by_duty[c["duty"]].append(c)
    for duty, dc in sorted(by_duty.items()):
        # 跨样本公共子串: 取每个文本单样本频次 + 跨样本交集候选
        counters = [freq_substrings(c["text"]) for c in dc]
        base = counters[0]
        for cnt in counters[1:]:
            base = collections.Counter({k: v for k, v in base.items() if k in cnt})
        if not base:
            # 降级: 每样本首个高频子串 (保守 needle)
            for c in dc:
                cnt = freq_substrings(c["text"])
                if cnt:
                    needle = cnt.most_common(1)[0][0]
                    rules.append({"duty": duty, "text": c["text"], "needle": needle,
                                  "value": c["value"], "a0": c["a0"], "a1": c["a1"],
                                  "conf": c["conf"], "param": c["param"]})
            continue
        # 公共子串集按"总频率"排序选 needle; 每 needle 取对应样本的众数期望
        used = set()
        for needle, _freq in base.most_common(30):
            if len(needle) < 3 or any(n in used for n in used if needle in n or n in needle):
                continue
            hits = [c for c in dc if needle in c["text"]]
            if not hits:
                continue
            vals = collections.Counter((c["value"], c["a0"], c["a1"]) for c in hits)
            (v, a0, a1), n = vals.most_common(1)[0]
            rules.append({"duty": duty, "text": needle, "needle": needle,
                          "value": v, "a0": a0, "a1": a1,
                          "conf": max(c["conf"] for c in hits), "param": 0})
            used.add(needle)
            if sum(1 for c in dc if any(n in c["text"] for n in used)) >= len(dc):
                break
    return rules


def simulate(rules, cases):
    covered = kept = 0
    for c in cases:
        hit = None
        for r in rules:
            if r["duty"] != c["duty"]:
                continue
            if r["needle"] in c["text"]:
                hit = r
                break
        if hit is not None:
            covered += 1
            if hit["value"] == c["value"] and hit["a0"] == c["a0"] and hit["a1"] == c["a1"]:
                kept += 1
    n = len(cases)
    return (100.0 * covered / n if n else 100.0,
            100.0 * kept / n if n else 100.0)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--in", dest="inp", default=os.path.join(ROOT, "sdk", "rulebook", "train_cases_w23.json"))
    ap.add_argument("--out", default=os.path.join(ROOT, "sdk", "rulebook", "inducted.json"))
    a = ap.parse_args()
    cases = json.load(open(a.inp))["cases"]
    rules = induce(cases)
    cov, keep = simulate(rules, cases)
    print(f"[induct] rules={len(rules)} coverage={cov:.0f}% fidelity={keep:.0f}%", flush=True)
    for r in rules:
        print(f"[induct]   duty={r['duty']} needle={r['needle']!r} -> {r['value']}", flush=True)
    json.dump(rules, open(a.out, "w"), indent=1)
    print(f"[induct] wrote {a.out}", flush=True)
    return 0 if keep == 100.0 else 1


if __name__ == "__main__":
    sys.exit(main())
