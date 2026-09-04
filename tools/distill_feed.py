#!/usr/bin/env python3
"""distill_feed.py — W23: 蒸馏候选收集 (自监督/实测反馈 -> train_cases 增量)

输入: m141 (模型在线) 的串口日志, 候选行:
    m141: T3 cand anom-novel '<text>' -> <got> gt=<gt> HIT|miss
    m141: T3 cand cls-novel  '<text>' -> <got> gt=<gt> HIT|miss
只收集 HIT 且规则未覆盖的样本 (get=规则未命中 -> 新增规则候选)。

输出: sdk/rulebook/train_cases_w23.json
    {"cases": [{duty, text, value, a0, a1, conf, param}...]}
    duty: 2=anom 1=classify; value=gt; conf=80 (高保真); param=0。

用法: python tools/distill_feed.py --log <qemu.log> [--out ...]
随后 (7B 归纳 bake 流程, 见 docs/84):
    python tools/distill_rules.py --online --in <merged.json> --out sdk/rulebook/fjru.bin
    # 把归纳结果 bake 进 distill_rules.py BAKED (离线确定性), 保真度 100% 门校验
"""
import argparse
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LINE_RE = re.compile(
    r"m141: T3 cand (anom-novel|cls-novel) '([^']*)' -> (\d+) gt=(\d+) (\w+)"
)
DUTY = {"anom-novel": 2, "cls-novel": 1}


def collect(log_text: str):
    hits = []
    for m in LINE_RE.finditer(log_text):
        kind, text, got, gt, verdict = m.groups()
        if verdict != "HIT":
            continue
        duty = DUTY[kind]
        # 去重交归纳步骤 (7B 泛化); 收集全部 HIT 保留闭环完整轨迹
        hits.append({"duty": duty, "text": text, "value": int(gt),
                     "a0": 0, "a1": 0, "conf": 80, "param": 0})
    return hits


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--log", required=True)
    ap.add_argument("--out", default=os.path.join(ROOT, "sdk", "rulebook", "train_cases_w23.json"))
    a = ap.parse_args()
    txt = open(a.log, errors="replace").read()
    hits = collect(txt)
    print(f"[feed] candidates={len(hits)}", flush=True)
    for c in hits:
        print(f"[feed]   duty={c['duty']} text={c['text']!r} value={c['value']}", flush=True)
    json.dump({"cases": hits}, open(a.out, "w"), indent=1)
    print(f"[feed] wrote {a.out}", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
