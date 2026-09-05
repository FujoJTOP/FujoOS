#!/usr/bin/env python3
"""box_server.py — FujoOS BOX-BRIDGE v0 宿主端 (W36/B-2)

盒 = 内核外供应商: 连接 QEMU COM2 (FJBOX:REQ 触发线) + monitor (pmemsave 读
共享页请求帧), 执行 4 个动词 (hash/info/size/echo —— 宿主 FOSS 流程, D1:
契约开源, 实现可闭源), 回包经 COM2 行 (RSP/DATA/END)。

协议 (与 kernel/src/boxbridge.rs 对齐):
  << FJBOX:REQ <seq> <len>            (触发线)
     -> pmemsave 0xA00000 0x430. 帧: payload @0x30 = verb(1B)+arg
  >> FJBOX:RSP <seq> 1
  >> FJBOX:DATA <seq> <off> <n> <text>   (text 到行尾; 每块 ≤64B)
  >> FJBOX:END <seq>

模式: normal(正常) | badart(产物带 ELF 魔数 -> 内核检疫拒收) |
      adapter(schema 违约 -> 内核列2a 记败)。

运行: python tools/box_server.py [--port 4001] [--mon 4568] [--mode normal]
环境: FUJO_LINK_PORT / FUJO_MON_PORT 同 argv。
"""
import argparse
import hashlib
import os
import socket
import sys
import tempfile
import time

HOST = "127.0.0.1"
PORT = int(os.environ.get("FUJO_LINK_PORT", "4001"))
MON_PORT = int(os.environ.get("FUJO_MON_PORT", "4568"))
SHM_BASE = 0xA00000
SHM_DUMP_LEN = 0x430  # 头 0x30 + payload ≤0x400
SHM_OFF_PAYLOAD = 0x30


def verb_name(v):
    return {1: "hash", 2: "info", 3: "size", 4: "echo"}.get(v, f"v{v}")


def run_verb(verb, arg):
    """动词 -> 产物 (宿主 FOSS 流程; adapter 契约: 输出 schema 由内核校验)。"""
    if verb == 1:  # hash: sha256 hex64
        return hashlib.sha256(arg.encode()).hexdigest()
    if verb == 2:  # info: 文本类型 (宿主无 `file` 时给确定输出)
        try:
            with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
                f.write(arg)
                path = f.name
            out = os.popen(f'file -b "{path}" 2>nul || file -b {path}').read().strip()
            os.unlink(path)
            if out:
                return out
        except OSError:
            pass
        return "ASCII text" if arg.isascii() else "Unicode text"
    if verb == 3:  # size: 字节数
        return str(len(arg.encode()))
    if verb == 4:  # echo: 原样回 (schema: 与 arg 全等)
        return arg
    return ""


class BoxServer:
    """线程化宿主盒 (import 由 fujoregress 使用)。"""

    def __init__(self, port=PORT, mon=MON_PORT, mode="normal"):
        self.port = port
        self.mon = mon
        self.mode = mode
        self.stop = False

    def _mon_cmd(self, s, cmd):
        s.sendall((cmd + "\n").encode())
        buf = b""
        end = time.time() + 4
        while time.time() < end:
            s.settimeout(0.5)
            try:
                d = s.recv(8192)
            except socket.timeout:
                continue
            if not d:
                break
            buf += d
            if b"(qemu) " in buf:
                break
        return buf

    def shm_read_frame(self, mon):
        """pmemsave -> (seq, verb, arg)。"""
        mon.sendall(f"pmemsave {SHM_BASE:#x} {SHM_DUMP_LEN:#x} _boxtmp.pmem\n".encode())
        buf = b""
        end = time.time() + 4
        while time.time() < end:
            mon.settimeout(0.5)
            try:
                d = mon.recv(8192)
            except socket.timeout:
                continue
            if not d:
                break
            buf += d
            if b"(qemu) " in buf:
                break
        try:
            data = open("_boxtmp.pmem", "rb").read()
            os.unlink("_boxtmp.pmem")
        except OSError:
            return None
        if len(data) < 0x30:
            return None
        seq = int.from_bytes(data[0x08:0x10], "little")
        n = min(int.from_bytes(data[0x14:0x18], "little"), 0x400)
        p = data[SHM_OFF_PAYLOAD:SHM_OFF_PAYLOAD + n]
        if not p:
            return None
        return seq, p[0] & 0xFF, p[1:].decode("utf-8", "replace")

    def serve_transport(self, sock):
        """proc/回调: 处理一段会话 (fujoregress 直接线程化)。"""
        buf = b""
        sock.settimeout(0.2)
        # monitor 连接 (独立 telnet; 读帧用)
        mon = socket.create_connection((HOST, self.mon), timeout=3)
        mon.settimeout(1.0)
        try:
            time.sleep(0.2)
            mon.recv(8192)
        except OSError:
            pass
        end = time.time() + 120
        while not self.stop and time.time() < end:
            try:
                d = sock.recv(4096)
            except socket.timeout:
                continue
            except OSError:
                break
            if not d:
                break
            buf += d
            while b"\n" in buf:
                line, buf = buf.split(b"\n", 1)
                line = line.strip()
                if not line.startswith(b"FJBOX:REQ "):
                    continue
                parts = line.split()
                if len(parts) < 3:
                    continue
                seq = int(parts[1])
                fr = self.shm_read_frame(mon)
                if fr is None:
                    sock.sendall(b"FJBOX:END %d\n" % seq)
                    continue
                fseq, verb, arg = fr
                seq = fseq  # 应答最新帧
                if self.mode == "badart":
                    prod = b"\x7fELF\x01\x02\x03bad"
                elif self.mode == "adapter":
                    prod = "x" * 64 if verb != 1 else "NOTHEX" * 6
                    prod = prod.encode()
                else:
                    prod = run_verb(verb, arg).encode()
                sock.sendall(b"FJBOX:RSP %d 1\n" % seq)
                off = 0
                while off < len(prod):
                    chunk = prod[off:off + 64]
                    sock.sendall(b"FJBOX:DATA %d %d %d %s\n"
                                 % (seq, off, len(chunk), chunk))
                    off += len(chunk)
                sock.sendall(b"FJBOX:END %d\n" % seq)
                print(f"[box] '{verb_name(verb)}' seq={seq} -> {len(prod)}B "
                      f"(mode={self.mode})", flush=True)
        try:
            mon.close()
        except OSError:
            pass

    def run(self):
        """独立运行 (main): 连 QEMU COM2 + monitor 服务。"""
        s = socket.create_connection((HOST, self.port), timeout=5)
        print(f"[box] connected com2 :{self.port} mode={self.mode}", flush=True)
        try:
            self.serve_transport(s)
        finally:
            s.close()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=PORT)
    ap.add_argument("--mon", type=int, default=MON_PORT)
    ap.add_argument("--mode", default="normal",
                    choices=["normal", "badart", "adapter"])
    a = ap.parse_args()
    BoxServer(a.port, a.mon, a.mode).run()


if __name__ == "__main__":
    main()
