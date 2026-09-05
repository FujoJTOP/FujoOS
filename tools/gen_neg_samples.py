#!/usr/bin/env python3
"""gen_neg_samples.py — B22: 负样本集 (判别归纳用, 专治过泛 needle)。

背景 (A4/W33): induct_rules v1 的公共子串归纳对候选集 fidelity 100%,
但 "ev "->1 这类过泛 needle 会把一切 "ev pid=.. rate=3 wr=ok" 正常事件判 1。
B22: 归纳器必须约束 "needle 不命中任何负样本" —— 本脚本确定性生成
anom(duty=2) 负样本 (value=0):
  A) known 负 (rate<10 wr=ok|1, 规则正确判0): 20 条
  B) novel-neg (rate 40-58 wr=ok): 20 条
  C) 判别边界 (rate=9x 但 wr=ok/1 -> 0; 专治 'rate=99'->1 过泛): 10 条
用法: python tools/gen_neg_samples.py [--out sdk/rulebook/neg_samples_w34.json]
"""
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def gen():
    cases = []
    for i in range(20):  # A known 负
        rate = 1 + (i % 9)
        wr = "ok" if i % 2 == 0 else "1"
        cases.append({"duty": 2, "text": f"ev pid={i} rate={rate} wr={wr}", "value": 0,
                      "a0": 0, "a1": 0, "conf": 90, "param": 0})
    for i in range(50):  # B novel-neg (rate 40-89 全段, 封死 'rate=7x/8x' 过泛)
        rate = 40 + i
        cases.append({"duty": 2, "text": f"ev pid={20 + i} rate={rate} wr=ok", "value": 0,
                      "a0": 0, "a1": 0, "conf": 90, "param": 0})
    for i in range(10):  # C 判别边界 (rate=9x 无死词)
        cases.append({"duty": 2, "text": f"ev pid={40 + i} rate={90 + (i % 9)} wr=ok",
                      "value": 0, "a0": 0, "a1": 0, "conf": 90, "param": 0})
    return cases


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        ROOT, "sdk", "rulebook", "neg_samples_w34.json")
    json.dump({"cases": gen()}, open(out, "w"), indent=1)
    print(f"[neg] wrote {out} ({len(gen())} samples)")


if __name__ == "__main__":
    main()
