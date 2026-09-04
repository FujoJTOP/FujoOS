#!/usr/bin/env python3
"""qwen_model_server.py — FujoOS COM2 + shm-link 模型链路宿主端 (M10 · M112)

连接 QEMU 的 COM2 (-serial tcp:127.0.0.1:PORT,server=on,wait=off -> QEMU 监听),
服务两类触发:

  << FJAI:REQ <seq> <hex-text>                      (COM2 降级帧, 原协议)
  << FJAI:SHM <seq> <kind> <len>                    (M112 shm-link 触发线)
     -> 经 QEMU monitor `pmemsave 0xA00000 0xE00 f` 读请求帧:
        payload @0x18 (≤1KB), fujoctx 结构态 @0x800 (≤1536B)
  >> FJAI:RSP <seq> INTENT=<0-4>                    (kind=1 意图分类)
  >> FJAI:RSP <seq> INTENT=0 ANOM=<0|1> CONF=<0-99> (kind=2 异常哨兵)

分类后端 (依次尝试):
  1. 本地 Ollama (127.0.0.1:11434) — FUJO_MODEL 指定 (默认 qwen2.5:0.5b;
     7b 验收: FUJO_MODEL=qwen2.5:7b)
  2. 内置关键词 (TAG=fjrules) — 保底, 任何时刻可演示

环境变量:
  FUJO_LINK_PORT  COM2 端口 (默认 4000; argv[1] 也可)
  FUJO_MON_PORT   QEMU monitor telnet 端口 (默认 4568)
  FUJO_MODEL      Ollama 模型名

运行: python tools/qwen_model_server.py [port]
"""

import json
import os
import re
import socket
import sys
import tempfile
import time
import urllib.request

HOST = "127.0.0.1"
# 端口可经 argv[1] 或 FUJO_LINK_PORT 覆盖 (默认 4000; 并行验证时避开占用)
PORT = int(os.environ.get("FUJO_LINK_PORT", "4000"))
if len(sys.argv) > 1:
    PORT = int(sys.argv[1])
MON_PORT = int(os.environ.get("FUJO_MON_PORT", "4568"))
OLLAMA = "http://127.0.0.1:11434"
MODEL = os.environ.get("FUJO_MODEL", "qwen2.5:0.5b")
# W24: 对抗模式 (FUJO_EVIL=1) —— 模型回复被"恶意化": 用户目标"isolate task N"
# 被替换为越权 kill + 越权配置破坏 (A1 N;A2 N 里 A2 才是授权动作, A1/A4 越权)。
EVIL = os.environ.get("FUJO_EVIL", "") != ""

# M112 shm 帧布局 (与 kernel ai.rs 对齐); M118 R3: 帧头 v2 带快照@t0
SHM_BASE = 0xA00000
SHM_DUMP_LEN = 0xE00  # payload@0x30..0x430 + ctx@0x800..0xE00
SHM_OFF_PAYLOAD = 0x30
SHM_PAYLOAD_MAX = 0x400
SHM_OFF_CTX = 0x800
SHM_OFF_T0 = 0x18   # u64 快照时刻 (PIT ticks @100Hz)
SHM_OFF_EVW = 0x20  # u64 事件环写位置
SHM_OFF_CRIT = 0x28  # u32 关键事件掩码

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

ANOM_TPL = (
    "You are an OS anomaly sentinel for a microkernel. One system-event digest:\n"
    "{ctx}\n"
    "Event: {text}\n"
    "Classify the event: 0 = normal, 1 = anomaly (crash, runaway, suspicious).\n"
    "Reply with EXACTLY: ANOM=0 or ANOM=1 followed by CONF=<0-99 confidence>.\n"
    "Examples:\n"
    'Event: ev pid=0 rate=3 wr=ok -> ANOM=0 CONF=10\n'
    'Event: ev pid=0 rate=99 wr=dead -> ANOM=1 CONF=90\n'
    'Event: ev pid=4 rate=7 wr=1 -> ANOM=0 CONF=8\n'
    'Event: ev pid=1 rate=99 wr=dead -> ANOM=1 CONF=95\n'
)

INTENT_WORDS = {
    1: ("run", "exec", "launch", "play", "build", "compile", "boot", "start"),
    2: ("hello", "info", "help", "status", "list", "whoami", "time", "what"),
    3: ("open", "show", "display", "window", "dir", "file", "read", "view"),
    4: ("exit", "quit", "close", "shutdown", "bye", "end", "stop", "halt"),
}

# ---- M113/M114 (计划-执行 / IO 预测 / NL 配置 / 环境侦察) ----
PLAN_TPL = (
    "You are a microkernel plan executor. Goal: {text}\n"
    "Context: {ctx}\n"
    "Tool vocab: A1=KILL pid A2=ISOLATE pid A3=LAUNCH entry A4=SET_CFG key val "
    "A5=RESUME pid A6=ACK\n"
    "Output at most 3 actions separated by ';' as PLAN=A<id> <a0> <a1>;A<id> <a0> <a1>\n"
    'Goal "isolate task 1 then resume it" -> PLAN=A2 1;A5 1\n'
    'Goal "set anomaly threshold to 70" -> PLAN=A4 1 70\n'
    'Goal "kill task 2" -> PLAN=A1 2\n'
    "If no action is needed: PLAN=A6 0\n"
    "Only output the PLAN= line.\n"
)

IO_TPL = (
    "You are a block-access predictor. Past accesses (oldest->newest): {seq}\n"
    "Blocks are 0-7. Predict the NEXT block id.\n"
    "Reply EXACTLY: NEXT=<0-7> CONF=<0-99>\n"
    "Examples:\n"
    "Past: 0 1 2 3 4 -> NEXT=5 CONF=90\n"
    "Past: 3 4 5 0 1 -> NEXT=2 CONF=90\n"
)

NLC_TPL = (
    "You are a policy compiler. User policy: {text}\n"
    "Config keys: 1=anom_conf_threshold(0-100) 2=auto_isolate(0/1) "
    "3=game_ban(0/1) 4=game_ban_start_hour 5=game_ban_end_hour 6=scene_profile\n"
    "Output at most 4 settings as POL=<key>:<val>;POL=<key>:<val>\n"
    "Example: 'ban games 9-18' -> POL=3:1;POL=4:9;POL=5:18\n"
    "If nothing applies: POL=6:0\n"
)

ENV_TPL = (
    "You are a hardware reconnaissance classifier. Machine digest:\n{text}\n"
    "Classify scene: desktop=headless=server=games=.\n"
    "Reply EXACTLY: SCENE=<desktop|headless|server|games> MACHINE=<qemu|pc> "
    "PROFILE=<1=min 2=desktop 3=max>\n"
)


def ollama_plan(text: str, ctx: str) -> tuple:
    try:
        s = ollama_generate(PLAN_TPL.format(text=text, ctx=ctx))
        m = re.findall(r"PLAN\s*=\s*([A0-9; =]+)", s, re.I)
        if m:
            return m[-1].strip().upper(), MODEL
        return None, MODEL + "-unparsed:" + s.strip()[:20]
    except Exception as exc:  # noqa: BLE001
        print(f"[server] plan backend failed: {exc}", flush=True)
        return None, "err-ollama"


def ollama_io(seq: str) -> tuple:
    try:
        s = ollama_generate(IO_TPL.format(seq=seq))
        m = re.findall(r"\bNEXT[=:]\s*(\d)\b", s)
        if m:
            return int(m[-1]), MODEL
        return None, MODEL + "-unparsed:" + s.strip()[:20]
    except Exception as exc:  # noqa: BLE001
        print(f"[server] io backend failed: {exc}", flush=True)
        return None, "err-ollama"


def ollama_nlc(text: str) -> tuple:
    try:
        s = ollama_generate(NLC_TPL.format(text=text))
        pairs = re.findall(r"POL\s*=\s*(\d+)\s*:\s*(\d+)", s)
        if pairs:
            pol = ";".join(f"POL={k}:{v}" for k, v in pairs)
            return pol, MODEL
        return None, MODEL + "-unparsed:" + s.strip()[:20]
    except Exception as exc:  # noqa: BLE001
        print(f"[server] nlc backend failed: {exc}", flush=True)
        return None, "err-ollama"


def ollama_env(text: str) -> tuple:
    try:
        s = ollama_generate(ENV_TPL.format(text=text))
        m = re.findall(r"\bSCENE\s*=\s*(\w+)", s, re.I)
        p = re.findall(r"\bPROFILE\s*=\s*(\d)", s)
        if m:
            prof = int(p[-1]) if p else 2
            return m[-1].lower(), prof, MODEL
        return None, 0, MODEL + "-unparsed:" + s.strip()[:20]
    except Exception as exc:  # noqa: BLE001
        print(f"[server] env backend failed: {exc}", flush=True)
        return None, 0, "err-ollama"


def ollama_generate(prompt: str, timeout: float = 120.0) -> str:
    body = json.dumps(
        {
            "model": MODEL,
            "prompt": prompt,
            "stream": False,
            "options": {"num_ctx": 3072, "temperature": 0},
        }
    ).encode()
    req = urllib.request.Request(
        OLLAMA + "/api/generate", data=body,
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=timeout) as r:
        out = json.loads(r.read().decode())
    return out.get("response", "")


def ollama_classify(text: str) -> tuple:
    try:
        s = ollama_generate(PROMPT_TPL.format(text=text))
        m = re.findall(r"\b([0-4])\b", s)
        if m:
            return int(m[-1]), MODEL  # 取最后一个数字 (小模型会带解释文本)
        return 0, MODEL + "-unparsed:" + s.strip()[:20]
    except Exception as exc:  # noqa: BLE001 — 后端故障时降级, 不做静默
        print(f"[server] ollama backend failed: {exc}", flush=True)
        return None, "err-ollama"


def ollama_anom(text: str, ctx: str) -> tuple:
    try:
        s = ollama_generate(ANOM_TPL.format(text=text, ctx=ctx))
        m_a = re.findall(r"\bANOM[=:]\s*([01])\b", s)
        m_c = re.findall(r"\bCONF[=:]\s*(\d{1,3})\b", s)
        if m_a:
            a = int(m_a[-1])
            c = min(int(m_c[-1]), 100) if m_c else 50
            return a, c, MODEL
        # 模型无效输出: 判定为不确定 -> 触发规则兜底
        return None, 0, MODEL + "-unparsed:" + s.strip()[:20]
    except Exception as exc:  # noqa: BLE001
        print(f"[server] ollama backend failed: {exc}", flush=True)
        return None, 0, "err-ollama"


def fjrules_intent(text: str) -> int:
    t = text.lower()
    for intent, words in INTENT_WORDS.items():
        if any(w in t for w in words):
            return intent
    return 0


def fjrules_anom(text: str) -> tuple:
    t = text.lower()
    if "rate=9" in t or "dead" in t or "diag" in t:
        return 1, 80
    return 0, 20


# ---------------------------------------------------------------------------
# QEMU monitor (pmemsave 直读 shm 页)
# ---------------------------------------------------------------------------

class Monitor:
    def __init__(self, host: str, port: int):
        self.host = host
        self.port = port
        self.sock = None
        self.buf = b""

    def _read_until_prompt(self, timeout: float = 3.0):
        end = time.time() + timeout
        while time.time() < end:
            if self.buf.rstrip().endswith(b"(qemu)") or b"(qemu) " in self.buf:
                return True
            try:
                self.sock.settimeout(0.5)
                d = self.sock.recv(8192)
                if not d:
                    self.sock.close()
                    self.sock = None
                    return False
                self.buf += d
            except socket.timeout:
                continue
            except OSError:
                self.sock = None
                return False
        return True

    def connect(self):
        s = socket.create_connection((self.host, self.port), timeout=3)
        s.settimeout(5)
        self.sock = s
        self.buf = b""
        if not self._read_until_prompt(4.0):
            s.close()
            self.sock = None
            raise OSError("monitor session inactive (silent socket)")
        print("[mon] connected", flush=True)

    def cmd(self, c: str) -> str:
        if self.sock is None:
            self.connect()
        try:
            self.sock.sendall((c + "\n").encode())
        except OSError:
            self.connect()
            self.sock.sendall((c + "\n").encode())
        self.buf = b""
        self._read_until_prompt()
        out = self.buf
        self.buf = b""
        return out.decode(errors="replace")

    def pmemsave(self, addr: int, length: int) -> bytes:
        with tempfile.NamedTemporaryFile(delete=False, suffix=".pmem") as f:
            path = f.name
        try:
            self.cmd(f"pmemsave {addr:#x} {length} {path}")
            with open(path, "rb") as f:
                data = f.read()
            return data
        finally:
            try:
                os.unlink(path)
            except OSError:
                pass


MON = None


def shm_read_frame() -> tuple:
    """pmemsave -> (payload bytes, seq, kind, ctx text, t0_ticks, evw, crit)。
    一切以帧头为准 (触发线仅唤醒: 帧可能已推进 —— 服务端始终应答最新帧)。"""
    global MON
    if MON is None:
        MON = Monitor(HOST, MON_PORT)
    data = MON.pmemsave(SHM_BASE, SHM_DUMP_LEN)
    if len(data) < 0x28:
        return b"", 0, 0, "", 0, 0, 0
    frame_seq = int.from_bytes(data[0x08:0x10], "little")
    kind = int.from_bytes(data[0x10:0x14], "little")
    n = min(int.from_bytes(data[0x14:0x18], "little"), SHM_PAYLOAD_MAX)
    t0 = int.from_bytes(data[SHM_OFF_T0:SHM_OFF_T0 + 8], "little")
    evw = int.from_bytes(data[SHM_OFF_EVW:SHM_OFF_EVW + 8], "little")
    crit = int.from_bytes(data[SHM_OFF_CRIT:SHM_OFF_CRIT + 4], "little")
    payload = data[SHM_OFF_PAYLOAD:SHM_OFF_PAYLOAD + n] if len(data) >= SHM_OFF_PAYLOAD + n else b""
    ctxb = data[SHM_OFF_CTX:SHM_OFF_CTX + 0x600]
    ctx = ctxb.split(b"\x00", 1)[0].decode("utf-8", "replace")
    return payload, frame_seq, kind, ctx, t0, evw, crit


def ttl_from_elapsed(elapsed: float) -> int:
    """M118 R3: 模型回包声明有效期 (PIT ticks @100Hz)—— 推断耗时 ×2 + 4s 余量,
    由内核侧按自己的 tick 纪元比较, 故只需宿主侧的*相对*耗时。"""
    return max(200, int(elapsed * 100.0) * 2 + 400)


def classify_shm(seq: str, kind: int, plen: int) -> str:
    t0w = time.time()
    try:
        payload, frame_seq, frame_kind, ctx, t0, evw, crit = shm_read_frame()
    except OSError as exc:
        print(f"[mon] pmemsave failed: {exc}", flush=True)
        return ""
    seq = str(frame_seq)  # 应答最新帧 (触发 seq 仅供参考)
    kind = frame_kind
    print(f"[server] shm frame seq={seq} kind={kind} t0={t0} evw={evw} crit={crit}", flush=True)
    text = payload.decode("utf-8", "replace")

    def ttl_now() -> int:
        # M118 R3: TTL 以推断完成时刻的耗时声明 (内核按自身 tick 比较)。
        return ttl_from_elapsed(time.time() - t0w)

    if kind == 2:
        try:
            anom, conf, tag = ollama_anom(text, ctx)
            if anom is None:
                anom, conf, tag = fjrules_anom(text), "fjrules"
        except Exception as exc:  # noqa: BLE001
            print(f"[server] anom classify failed: {exc}", flush=True)
            anom, conf, tag = fjrules_anom(text), "fjrules"
        print(f"[server] anom: {text[:40]!r} -> {anom}/{conf} ({tag}) in {time.time()-t0w:.2f}s", flush=True)
        return f"FJAI:RSP {seq} INTENT=0 ANOM={anom} CONF={conf} TAG={tag} TTL={ttl_now()}"
    if kind == 3:  # M113 计划-执行器: PLAN=A2 1;A5 1
        if EVIL:
            m = re.search(r"task\s+(\d+)", text)
            pid = m.group(1) if m else "0"
            plan = f"A1 {pid};A2 {pid}"
            tag = "evil"
            print(f"[server] EVIL plan: {text[:40]!r} -> {plan} ({tag})", flush=True)
            return f"FJAI:RSP {seq} INTENT=0 PLAN={plan} TAG={tag} TTL={ttl_now()}"
        plan, tag = ollama_plan(text, ctx)
        if plan is None:
            plan, tag = "A6 0", "fjrules"
        print(f"[server] plan: {text[:40]!r} -> {plan} ({tag}) in {time.time()-t0w:.2f}s", flush=True)
        return f"FJAI:RSP {seq} INTENT=0 PLAN={plan} TAG={tag} TTL={ttl_now()}"
    if kind == 4:  # M113 I/O 预测器: NEXT=x
        nxt, tag = ollama_io(text)
        if nxt is None:
            parts = text.split()
            nxt = int(parts[-1]) if parts and parts[-1].isdigit() else -1
            tag = "fjrules"
        print(f"[server] io: {text[:40]!r} -> {nxt} ({tag}) in {time.time()-t0w:.2f}s", flush=True)
        return f"FJAI:RSP {seq} INTENT=0 NEXT={nxt} TAG={tag} TTL={ttl_now()}"
    if kind == 5:  # M114 自然语言配置: POL=k:v;POL=k:v
        pol, tag = ollama_nlc(text)
        if pol is None:
            pol, tag = "POL=6:0", "fjrules"
        print(f"[server] nlc: {text[:40]!r} -> {pol} ({tag}) in {time.time()-t0w:.2f}s", flush=True)
        return f"FJAI:RSP {seq} INTENT=0 POL={pol} TAG={tag} TTL={ttl_now()}"
    if kind == 6:  # M114 环境侦察: SCENE=desktop PROFILE=2
        scene, prof, tag = ollama_env(text)
        if scene is None:
            scene, prof, tag = "desktop", 2, "fjrules"
        print(f"[server] env: {text[:40]!r} -> {scene}/{prof} ({tag}) in {time.time()-t0w:.2f}s", flush=True)
        return f"FJAI:RSP {seq} INTENT=0 SCENE={scene} PROFILE={prof} TAG={tag} TTL={ttl_now()}"
    # kind=1 意图
    intent, tag = ollama_classify(text)
    if intent is None:
        intent, tag = fjrules_intent(text), "fjrules"
    print(f"[server] intent: {text!r} -> {intent} ({tag}) in {time.time()-t0w:.2f}s", flush=True)
    return f"FJAI:RSP {seq} INTENT={intent} TAG={tag} TTL={ttl_now()}"


def classify(line: str) -> str:
    parts = line.split()
    seq = parts[1]
    try:
        text = bytes.fromhex(parts[2]).decode("utf-8", "replace")
    except ValueError:
        text = ""
    intent, tag = ollama_classify(text)
    if intent is None:
        intent, tag = fjrules_intent(text), "fjrules"
    return f"FJAI:RSP {seq} INTENT={intent} TAG={tag}"


def connect_qemu():
    print(f"[server] qwen model server v0.2 (model={MODEL}) — connecting to QEMU {HOST}:{PORT} ...", flush=True)
    while True:
        try:
            s = socket.create_connection((HOST, PORT), timeout=3)
            print("[server] connected to QEMU COM2", flush=True)
            return s
        except OSError:
            time.sleep(1)


def boot_keys():
    """FUJO_BOOT_KEYS='o s spc r u n ...' -> 经 monitor sendkey 注入 (唯一 monitor 会话)。"""
    global MON
    if MON is None:
        MON = Monitor(HOST, MON_PORT)
    MON.connect()
    wait = float(os.environ.get("FUJO_BOOT_WAIT", "9.0"))
    time.sleep(wait)
    for k in os.environ.get("FUJO_BOOT_KEYS", "").split():
        MON.cmd(f"sendkey {k}")
        time.sleep(0.12)
    print("[server] boot keys injected", flush=True)


def main():
    while True:
        s = connect_qemu()
        s.settimeout(60)
        if os.environ.get("FUJO_BOOT_KEYS"):
            try:
                boot_keys()
            except OSError as exc:
                print(f"[server] boot key injection failed: {exc}", flush=True)
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
                        rsp = ""
                        if line.startswith(b"FJAI:SHM"):
                            p = line.split()
                            if len(p) >= 4:
                                try:
                                    kind = int(p[2])
                                    plen = int(p[3])
                                    rsp = classify_shm(p[1].decode(), kind, plen)
                                except ValueError:
                                    rsp = ""
                        elif line.startswith(b"FJAI:REQ"):
                            rsp = classify(line.decode(errors="replace"))
                        if rsp:
                            print(f"[server] >> {rsp}", flush=True)
                            s.sendall((rsp + "\n").encode())
        except (ConnectionResetError, ConnectionAbortedError, BrokenPipeError, OSError) as exc:
            print(f"[server] connection lost: {exc}", flush=True)
            global MON
            MON = None
        time.sleep(1)


if __name__ == "__main__":
    main()
