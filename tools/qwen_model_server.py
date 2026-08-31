#!/usr/bin/env python3
"""qwen_model_server.py — FujoOS COM2 模型链路宿主端 (M10 · engine=qwen)

连接 QEMU 的 COM2 (-serial tcp:127.0.0.1:4000,server=on,wait=off -> QEMU 监听),
服务 FJAI:REQ / FJAI:RSP 行协议:

    << FJAI:REQ <seq> <hex-text>
    >> FJAI:RSP <seq> INTENT=<0-4> TAG=<backend>

分类后端 (依次尝试):
  1. 本地 Ollama (127.0.0.1:11434) — qwen2.5:0.5b (用户指定的小模型)
  2. 内置关键词打分 (TAG=fjrules) — 保底, 保证链路任何时候可演示

运行: python tools/qwen_model_server.py
"""

import json
import os
import re
import socket
import sys
import time
import urllib.request

HOST = "127.0.0.1"
# 端口可经 argv[1] 或 FUJO_LINK_PORT 覆盖 (默认 4000; 并行验证时避开占用)
PORT = int(os.environ.get("FUJO_LINK_PORT", "4000"))
if len(sys.argv) > 1:
    PORT = int(sys.argv[1])
OLLAMA = "http://127.0.0.1:11434"
MODEL = "qwen2.5:0.5b"

PROMPT_TPL = (
    "Classify the user command. Reply with only one digit, nothing else.\n"
    "0=unknown  1=run/execute  2=query  3=open  4=exit\n"
    "Examples:\n"
    'Command: "run the game" -> 1\n'
    'Command: "build kernel" -> 1\n'
    'Command: "exit" -> 4\n'
    'Command: "quit" -> 4\n'
    'Command: "open file" -> 3\n'
    'Command: "hello" -> 2\n'
    'Command: "what time is it" -> 2\n'
    'Command: "xyzzy" -> 0\n'
    'Command: "{text}" ->\n'
)

INTENT_WORDS = {
    1: ("run", "exec", "launch", "play", "build", "compile", "boot", "start"),
    2: ("hello", "info", "help", "status", "list", "whoami", "time", "what"),
    3: ("open", "show", "display", "window", "dir", "file", "read", "view"),
    4: ("exit", "quit", "close", "shutdown", "bye", "end", "stop", "halt"),
}


def ollama_classify(text: str) -> tuple:
    try:
        body = json.dumps(
            {
                "model": MODEL,
                "prompt": PROMPT_TPL.format(text=text),
                "stream": False,
                "options": {"num_ctx": 2048, "temperature": 0},
            }
        ).encode()
        req = urllib.request.Request(
            OLLAMA + "/api/generate", data=body,
            headers={"Content-Type": "application/json"},
        )
        with urllib.request.urlopen(req, timeout=30) as r:
            out = json.loads(r.read().decode())
        s = out.get("response", "")
        m = re.findall(r"\b([0-4])\b", s)
        if m:
            return int(m[-1]), MODEL  # 取最后一个数字 (小模型会带解释文本)
        return 0, MODEL + "-unparsed:" + s.strip()[:20]
    except Exception as exc:  # noqa: BLE001 — 后端故障时降级, 不做静默
        print(f"[server] ollama backend failed: {exc}", flush=True)
        return None, "err-ollama"


def fjrules(text: str) -> int:
    t = text.lower()
    for intent, words in INTENT_WORDS.items():
        if any(w in t for w in words):
            return intent
    return 0


def classify(text: str) -> tuple:
    intent, tag = ollama_classify(text)
    if intent is None:
        intent, tag = fjrules(text), "fjrules"
    return intent, tag


def handle(line: str) -> str:
    parts = line.split()
    if len(parts) < 3:
        return ""
    seq, hx = parts[1], parts[2]
    try:
        text = bytes.fromhex(hx).decode("utf-8", "replace")
    except ValueError:
        text = ""
    intent, tag = classify(text)
    return f"FJAI:RSP {seq} INTENT={intent} TAG={tag}"


def connect_qemu():
    print(f"[server] qwen model server v0.1 — connecting to QEMU {HOST}:{PORT} ...", flush=True)
    while True:
        try:
            s = socket.create_connection((HOST, PORT), timeout=3)
            print("[server] connected to QEMU COM2", flush=True)
            return s
        except OSError:
            time.sleep(1)


def main():
    while True:
        s = connect_qemu()
        s.settimeout(60)
        buf = b""
        try:
            with s:
                while True:
                    try:
                        data = s.recv(4096)
                    except socket.timeout:
                        print("[server] read timeout (QEMU idle)", flush=True)
                        continue
                    if not data:
                        print("[server] QEMU closed connection", flush=True)
                        break
                    buf += data
                    while b"\n" in buf:
                        line, buf = buf.split(b"\n", 1)
                        line = line.strip()
                        if not line:
                            continue
                        print(f"[server] << {line.decode(errors='replace')}", flush=True)
                        if line.startswith(b"FJAI:REQ"):
                            rsp = handle(line.decode(errors="replace"))
                            print(f"[server] >> {rsp}", flush=True)
                            s.sendall((rsp + "\n").encode())
        except (ConnectionResetError, ConnectionAbortedError, BrokenPipeError, OSError) as exc:
            print(f"[server] connection lost: {exc}", flush=True)
        time.sleep(1)


if __name__ == "__main__":
    main()
