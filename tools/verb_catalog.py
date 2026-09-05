#!/usr/bin/env python3
"""verb_catalog.py — B-29: 动词词汇表枚举 (接口完备性实证)

FUFORALL 三级完备性之"接口完备性"证据: BOX-BRIDGE 动词集 = 任务动词白名单,
有限封顶 (v1 = 6 个), 每个动词带 (供应商平台, schema 谓词, 产物上界, 回归用例)。
输出: sdk/fixtures/verb_catalog.json + 本标准输出 (docs/111 引用)。

用法: python tools/verb_catalog.py [--json sdk/fixtures/verb_catalog.json]
"""
import argparse
import json
import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# 动词表: id -> {name, schema(机器可判定谓词), max, case, status, note}
VERBS = {
    1: dict(name="hash", schema="len==64 & [0-9a-f]{64}", max=64,
            case="box-online/box-v1-online", status="v0",
            note="sha256 hexdigest; 确定性 (golden 可复算)"),
    2: dict(name="info", schema="len>=4 & printable-ascii", max=512,
            case="box-online", status="v0",
            note="file(1) 等价; 宿主 fallback ASCII text"),
    3: dict(name="size", schema="[0-9]{1,20}", max=64,
            case="box-online", status="v0", note="wc -c 等价"),
    4: dict(name="echo", schema="== arg (全等回显)", max=128,
            case="box-online", status="v0",
            note="最强 schema: 输入输出不变式 (Adapter 契约)"),
    5: dict(name="file2pdf", schema="head %PDF- & tail %%EOF & ascii", max=3072,
            case="box-v1-online", status="v1",
            note="winword COM 优先 (真实 Windows 盒), 超限回退零依赖微 PDF"),
    6: dict(name="framebuf", schema="BMP 32x24 RGB24 结构 (54+2304=2358B)", max=3072,
            case="box-v1-online", status="v1",
            note="B-3 像素流通路版; 人类窗口呈现 = v2"),
}

# 候选动词 (声明性: 需要新基础设施才可加的, 防"无限回归"的诚实边界)
CANDIDATES = [
    dict(name="file2txt", need="大产物 (现有 fileread 侧可裁剪, 512B-3072B 内可加)",
         blocked="v1 可支持 (PDF 带外已通)"),
    dict(name="screen", need="GUI 帧流 v2 (人类窗口呈现)", blocked="像素流通路 v1 已通"),
    dict(name="dbquery", need="盒协议加结构化批量参数 (arg 128B 上限)", blocked="arg 槽扩展"),
]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default=os.path.join(ROOT, "sdk", "fixtures",
                                                   "verb_catalog.json"))
    a = ap.parse_args()
    doc = {
        "family": "BOX-BRIDGE verb vocabulary (B-29)",
        "providers": [{"id": 0, "platform": "host (Windows/WSL)", "domain": 1}],
        "verbs": {str(k): v for k, v in VERBS.items()},
        "candidates": CANDIDATES,
        "claim": ("接口完备性: 动词集 = 任务动词白名单 (有限封顶); "
                  "新动词无需新内核设施 (包装现有谓词/传输), "
                  "新设施类动词 (GUI 帧流/批量参数) 显式列入候选 = 声明边界。"),
    }
    os.makedirs(os.path.dirname(a.json), exist_ok=True)
    json.dump(doc, open(a.json, "w"), indent=1)
    print("B-29 verb catalog (interface completeness):")
    for k, v in sorted(VERBS.items(), key=lambda kv: int(kv[0])):
        print(f"  [{k}] {v['name']:10s} schema='{v['schema']}' max={v['max']:4d} "
              f"status=v{v['status'][1:]} case={v['case']}")
    print(f"  total: {len(VERBS)} finite verbs (+{len(CANDIDATES)} declared "
          f"candidates: {', '.join(c['name'] for c in CANDIDATES)})")
    print(f"  catalog: {a.json}")


if __name__ == "__main__":
    main()
