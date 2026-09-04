#!/usr/bin/env python3
"""distill_rules.py — W10 R5 策略蒸馏: 审计/实测记录 -> 7B 归纳 if-then 规则
-> 编译为内核可执行 FJRU v1 确定性字节码 + 保真度曲线 (docs/61)。

输入: sdk/rulebook/train_cases.json (M112-M115 7B 实测记录)
  case = {duty(1=classify 2=anom 3=plan 4=io 5=nlc 6=env), text,
          value, a0, a1, conf, param}
归纳: --online 用 Ollama 7B 泛化 (每个职责一组, 输出 needle; 校验保真度 100%
  否则回退); --offline/默认用已记录的 7B 归纳结果 (确定性, CI 可用)。
编译: FJRU v1 = magic"FJRU"|ver u32=1|count u16|pad u16|entries...
  entry = [nl u8][needle nlB][value u8][a0 u8][a1 u8][conf u8][param u8][duty u8]
输出: sdk/rulebook/fjru.bin + rulebook.h (demo 嵌入) + fidelity.csv (曲线)。
用法: python tools/distill_rules.py [--online] [--in ...] [--out ...]
                               [--header ...] [--fidelity ...]
"""
import argparse
import json
import os
import struct
import sys
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DUTY_NAME = {1: "classify", 2: "anom", 3: "plan", 4: "io", 5: "nlc", 6: "env"}

# ---- 已记录的 7B 归纳结果 (--online 校验后 bake; 保证 CI 确定性) ----
# 每项: (duty, needle, value, a0, a1, conf, param)
# W23: +4 条 (m141 novel 命中实测 bake: wr=memleak/zombie, launch/what);
# 精确 needle 必须置于 (2,"rate=") 通配之前 (rules_match 按序首个命中)。
BAKED = [
    (2, "wr=dead", 1, 80, 0, 80, 0),
    (2, "wr=memleak", 1, 80, 0, 80, 0),
    (2, "wr=zombie", 1, 80, 0, 80, 0),
    (2, "wr=diag", 1, 80, 0, 80, 0),
    (2, "rate=99", 1, 80, 0, 80, 0),
    (2, "rate=", 0, 20, 0, 20, 0),
    (2, "wr=ok", 0, 20, 0, 20, 0),
    (3, "isolate task", 2, 0, 0, 90, 1),
    (3, "kill task", 1, 0, 0, 90, 1),
    (3, "threshold", 4, 1, 70, 90, 0),
    (4, "1 2 3 4", 5, 0, 0, 90, 0),
    (4, "5 0 1", 2, 0, 0, 90, 0),
    (5, "ban games", 3, 1, 0, 90, 0),
    (1, "run", 1, 0, 0, 90, 0),
    (1, "open", 3, 0, 0, 90, 0),
    (1, "exit", 4, 0, 0, 90, 0),
    (1, "hello", 2, 0, 0, 90, 0),
    (1, "launch", 1, 0, 0, 90, 0),
    (1, "what", 2, 0, 0, 90, 0),
]

INDUCE_TPL = (
    "You distill OS AI rules from observed cases. For duty '{duty}', generalize "
    "these (input -> suggestion) pairs into as few deterministic if-then rules as "
    "possible (prefix/substring needles):\n{cases}\n"
    "Reply EXACTLY one rule per line, format: NEEDLE|VALUE|A0|A1|CONF|PARAM\n"
    "PARAM=1 means the parameter is the digits right after NEEDLE in the input "
    "(for task ids etc.). No prose.\n"
)


def ollama(prompt: str, model: str) -> str:
    body = json.dumps({"model": model, "prompt": prompt, "stream": False,
                       "options": {"num_ctx": 2048, "temperature": 0}}).encode()
    req = urllib.request.Request("http://127.0.0.1:11434/api/generate", data=body,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=180) as r:
        return json.loads(r.read().decode()).get("response", "")


def induce(cases):
    """7B 归纳每职责规则; 无法解析/保真不足回退 BAKED (数据真实来源, 见 docs/61)。"""
    out = []
    by_duty = {}
    for c in cases:
        by_duty.setdefault(c["duty"], []).append(c)
    for duty, dc in sorted(by_duty.items()):
        lines = "\n".join(f"input: {c['text']} -> {c['value']},{c['a0']},{c['a1']}"
                          f" conf={c['conf']} param={c['param']}" for c in dc)
        prompt = INDUCE_TPL.format(duty=DUTY_NAME.get(duty, duty), cases=lines)
        s = ollama(prompt, MODEL).strip()
        got = []
        for ln in s.splitlines():
            p = ln.split("|")
            if len(p) != 6:
                continue
            try:
                got.append((duty, p[0].strip(), int(p[1]), int(p[2]),
                            int(p[3]), int(p[4]), int(p[5])))
            except ValueError:
                continue
        if got and simulate(got, dc)[1] == 100.0:
            print(f"[induce] 7B duty={DUTY_NAME.get(duty)}: {len(got)} rules ok", flush=True)
            out += got
        else:
            print(f"[induce] duty={DUTY_NAME.get(duty)}: fallback baked", flush=True)
            out += [r for r in BAKED if r[0] == duty]
    return out


def simulate(rules, cases):
    """覆写率/保持率: 按规则顺序取首个匹配; 匹配且输出==case 期望则保持。"""
    covered = 0
    kept = 0
    for c in cases:
        hit = None
        for (d, needle, v, a0, a1, conf, param) in rules:
            if d not in (0, c["duty"]):
                continue
            pos = c["text"].find(needle)
            if pos < 0:
                continue
            if param:
                j = pos + len(needle)
                while j < len(c["text"]) and c["text"][j] == " ":
                    j += 1
                s = ""
                while j < len(c["text"]) and c["text"][j].isdigit():
                    s += c["text"][j]
                    j += 1
                a0 = int(s) if s else a0
            hit = (v, a0, a1, conf)
            break
        if hit is not None:
            covered += 1
            if hit[0] == c["value"] and hit[1] == c["a0"] and hit[2] == c["a1"]:
                kept += 1
    n = len(cases)
    return (100.0 * covered / n if n else 100.0,
            100.0 * kept / n if n else 100.0)


def compile_fjru(rules) -> bytes:
    hdr = struct.pack("<I I H H", 0x55524A46, 1, len(rules), 0) + bytes(4)  # 16B 头, 条目从 0x10
    body = b""
    for (duty, needle, v, a0, a1, conf, param) in rules:
        nb = needle.encode("ascii")
        if not (0 < len(nb) <= 40):
            raise ValueError(f"bad needle: {needle!r}")
        body += bytes([len(nb)]) + nb + bytes([v, a0, a1, conf, param, duty])
    return hdr + body


def emit_header(rules, path):
    with open(path, "w", newline="\n") as f:
        f.write("/* generated by tools/distill_rules.py — FJRU v1 rulebook (docs/61) */\n")
        f.write("#pragma once\n")
        f.write("static const unsigned char RULEBOOK[] = {\n")
        data = compile_fjru(rules)
        for i in range(0, len(data), 16):
            f.write("  " + ",".join(str(b) for b in data[i:i + 16]) + ",\n")
        f.write("};\n")
        f.write(f"static const unsigned int RULEBOOK_LEN = {len(data)};\n")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--online", action="store_true")
    ap.add_argument("--model", default="qwen2.5:7b")
    ap.add_argument("--in", dest="inp", default=os.path.join(ROOT, "sdk", "rulebook", "train_cases.json"))
    ap.add_argument("--out", default=os.path.join(ROOT, "sdk", "rulebook", "fjru.bin"))
    ap.add_argument("--header", default=os.path.join(ROOT, "sdk", "rulebook", "rulebook.h"))
    ap.add_argument("--fidelity", default=os.path.join(ROOT, "sdk", "rulebook", "fidelity.csv"))
    a = ap.parse_args()
    global MODEL
    MODEL = a.model

    cases = json.load(open(a.inp))["cases"]
    rules = induce(cases) if a.online else list(BAKED)
    cov, keep = simulate(rules, cases)
    print(f"[distill] rules={len(rules)} coverage={cov:.0f}% fidelity={keep:.0f}%", flush=True)
    if keep != 100.0 or cov != 100.0:
        print("[distill] WARNING: fidelity < 100% — refusing to emit", flush=True)
        return 1

    open(a.out, "wb").write(compile_fjru(rules))
    emit_header(rules, a.header)
    # 保真度曲线: 前 k 条规则 (按序) 的覆盖/保持
    with open(a.fidelity, "w", newline="\n") as f:
        f.write("k,coverage,fidelity\n")
        for k in range(1, len(rules) + 1):
            c, p = simulate(rules[:k], cases)
            f.write(f"{k},{c:.0f},{p:.0f}\n")
        print(f"[distill] wrote {a.out} / {a.header} / {a.fidelity}", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
