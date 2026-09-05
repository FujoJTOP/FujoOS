#!/usr/bin/env python3
"""box_server.py — FujoOS BOX-BRIDGE 宿主端 (W36/B-2 v0 · W37 v1)

盒 = 内核外供应商: 连接 QEMU COM2 (FJBOX:REQ 触发线) + monitor (pmemsave 读
共享页请求帧), 执行 6 个动词 (v0: hash/info/size/echo · v1: file2pdf/framebuf
—— 宿主 FOSS 流程, D1: 契约开源, 实现可闭源), 回包经 COM2 行 (RSP/DATA/END)。

协议 (与 kernel/src/boxbridge.rs 对齐):
  << FJBOX:REQ <seq> <len>            (触发线)
     -> pmemsave 0xA00000 0x430. 帧: payload @0x30 = verb(1B)+arg
  >> FJBOX:RSP <seq> 1
  >> FJBOX:DATA <seq> <off> <n> <text>   (text 到行尾; 每块 ≤64B)
  >> FJBOX:END <seq>

模式:
  normal  正常产物 (v0+v1 六动词)
  badart  产物带 ELF 魔数 -> 内核检疫拒收 (-2)
  adapter schema 违约 -> 内核列2a 记败 (-3)
  fuzz    6 种畸形产物轮换 (B-31 检疫门 fuzz; 全 -2/-3)
  --golden <json>  黄金轨迹校验 (B-30): 每次产物 sha256 比对, 不一致
                    -> 替换为违约文本 (内核 -3, demo FAIL 可见)

运行: python tools/box_server.py [--port 4001] [--mon 4568] [--mode normal]
环境: FUJO_LINK_PORT / FUJO_MON_PORT 同 argv。
"""
import argparse
import hashlib
import json
import os
import socket
import struct
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
    return {1: "hash", 2: "info", 3: "size", 4: "echo",
            5: "file2pdf", 6: "framebuf"}.get(v, f"v{v}")


# ---- W37 v1: 宿主侧 PDF/BMP 生成 (FOSS; winword COM 尝试 - 真实 Windows 盒面) ----

def micro_pdf(arg: str) -> bytes:
    """零依赖合法微型 PDF (纯 ascii, %PDF- 头 + %%EOF 尾; 契约内 ≤3072B)。"""
    txt = arg.replace("\\", "").replace("(", "").replace(")", "")
    body = (b"BT /F1 12 Tf 20 60 Td (" + txt.encode() + b") Tj ET")
    objs = {
        1: b"<</Type/Catalog/Pages 2 0 R>>",
        2: b"<</Type/Pages/Kids[3 0 R]/Count 1>>",
        3: b"<</Type/Page/Parent 2 0 R/MediaBox[0 0 300 100]/Contents 4 0 R"
           b"/Resources<</Font<</F1 5 0 R>>>>>>",
        4: b"<</Length " + str(len(body)).encode() + b">>stream\n" + body
           + b"\nendstream",
        5: b"<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>",
    }
    out = bytearray(b"%PDF-1.4\n")
    offs = [0]
    for k in sorted(objs):
        offs.append(len(out))
        out += b"%d 0 obj\n" % k + objs[k] + b"\nendobj\n"
    xref = len(out)
    out += b"xref\n0 6\n0000000000 65535 f \n"
    for o in offs[1:]:
        out += b"%010d 00000 n \n" % o
    out += (b"trailer<</Size 6/Root 1 0 R>>\nstartxref\n" + str(xref).encode()
            + b"\n%%EOF\n")
    return bytes(out)


_WW_PROBE = None  # B-2v1: winword COM 探活缓存 (None=未试, False=不可用)


def winword_pdf(arg: str) -> bytes:
    """真实 Windows 盒: Word COM 转 PDF (本机 Office 可用时; 产物 >3072 则拒)。"""
    global _WW_PROBE
    if _WW_PROBE is None:
        try:
            import subprocess
            r = subprocess.run(
                ["powershell", "-Command",
                 "try { $w = New-Object -ComObject Word.Application; $w.Quit(); "
                 "Write-Output OK } catch { Write-Output NO }"],
                capture_output=True, timeout=20)
            _WW_PROBE = b"OK" in r.stdout
            print(f"[box] winword COM probe: {_WW_PROBE}", flush=True)
        except Exception as exc:  # noqa: BLE001
            _WW_PROBE = False
            print(f"[box] winword COM probe failed: {exc}", flush=True)
    if not _WW_PROBE:
        return micro_pdf(arg)
    try:
        import subprocess
        ps = ("$w = New-Object -ComObject Word.Application; $w.Visible=$false;"
              "$d = $w.Documents.Add(); $d.Content.Text = %s;"
              "$d.SaveAs2('_box_tmp.pdf', 17); $w.Quit()" % json.dumps(arg))
        r = subprocess.run(["powershell", "-Command", ps], capture_output=True,
                           timeout=25)
        if r.returncode == 0 and os.path.exists("_box_tmp.pdf"):
            data = open("_box_tmp.pdf", "rb").read()
            os.unlink("_box_tmp.pdf")
            return data
    except Exception as exc:  # noqa: BLE001
        print(f"[box] winword COM convert failed: {exc}", flush=True)
    return micro_pdf(arg)


def make_bmp(w=32, h=24) -> bytes:
    """B-3 通路版像素帧: 32x24 RGB24 BMP (54 + w*h*3 = 2358B)。"""
    px = bytearray()
    for y in range(h):
        for x in range(w):
            px += bytes([(x * 7 + y * 3) % 256, (y * 9 + x * 5) % 256,
                         ((x + y) * 5) % 256])
    size = 54 + len(px)
    hdr = (b"BM" + struct.pack("<I", size) + b"\x00\x00\x00\x00" + struct.pack("<I", 54)
           + struct.pack("<I", 40)
           + struct.pack("<iI", w, h) + struct.pack("<HH", 1, 24) + b"\x00" * 24)
    return hdr + px


FUZZ_SEQ = [
    b"\x7fELF\x01\x02" + b"x" * 60,          # ELF 魔数 -> 检疫 -2
    b"MZ\x90\x00" + b"y" * 60,               # MZ 魔数 -> 检疫 -2
    b"\x01\x02\x03\x04" * 16,                # 非 ascii 控制符 -> 检疫 -2
    b"A" * 4096,                             # 超上限 (行数 >48) -> 检疫 -2
    b"%PDF-9.9 not a real pdf tail",         # PDF 头违例 -> schema -3
    b"BM" + b"\x00" * 232 + b"BAD" * 10,     # BMP 结构违例 -> schema -3
]


def run_verb(verb, arg):
    """动词 -> 产物 (宿主 FOSS 流程; adapter 契约: 输出 schema 由内核校验)。"""
    if verb == 1:  # hash: sha256 hex64
        return hashlib.sha256(arg.encode()).hexdigest().encode()
    if verb == 2:  # info: 文本类型 (宿主无 `file` 时给确定输出)
        try:
            with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
                f.write(arg)
                path = f.name
            out = os.popen(f'file -b "{path}" 2>nul || file -b {path}').read().strip()
            os.unlink(path)
            if out:
                return out.encode()
        except OSError:
            pass
        return ("ASCII text" if arg.isascii() else "Unicode text").encode()
    if verb == 3:  # size: 字节数
        return str(len(arg.encode())).encode()
    if verb == 4:  # echo: 原样回 (schema: 与 arg 全等)
        return arg.encode()
    if verb == 5:  # file2pdf (v1): winword COM -> 微 PDF (真盒优先, 契约 ≤3072)
        data = winword_pdf(arg)
        return data if len(data) <= 3072 else micro_pdf(arg)
    if verb == 6:  # framebuf (B-3 通路版): 32x24 BMP
        return make_bmp()
    return b""


class BoxServer:
    """线程化宿主盒 (import 由 fujoregress 使用)。"""

    def __init__(self, port=PORT, mon=MON_PORT, mode="normal", golden=None):
        self.port = port
        self.mon = mon
        self.mode = mode
        self.golden = golden
        self._fuzz_i = 0
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
                    prod = FUZZ_SEQ[0]
                elif self.mode == "adapter":
                    prod = b"NOTHEX" * 6 if verb == 1 else b"x" * 64
                elif self.mode == "fuzz":
                    prod = FUZZ_SEQ[self._fuzz_i % len(FUZZ_SEQ)]
                    self._fuzz_i += 1
                else:
                    prod = run_verb(verb, arg)
                # B-30 黄金轨迹: 产物 sha256 与 golden 表比对; 违约 -> 替换 (内核 -3)
                if self.golden and self.mode in ("normal", "golden"):
                    g = self.golden.get(str(verb))
                    if g:
                        cur = hashlib.sha256(prod).hexdigest()
                        if cur != g.get("sha256"):
                            prod = b"GOLDEN MISMATCH verb=" + str(verb).encode()
                            print(f"[box] GOLDEN MISMATCH verb={verb}", flush=True)
                    else:
                        prod = b"GOLDEN UNKNOWN VERB"
                        print(f"[box] GOLDEN UNKNOWN verb={verb}", flush=True)
                sock.sendall(b"FJBOX:RSP %d 1\n" % seq)
                off = 0
                while off < len(prod):
                    chunk = prod[off:off + 64]
                    sock.sendall(b"FJBOX:DATA %d %d %d %s\n"
                                 % (seq, off, len(chunk), chunk.hex().encode()))
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


def gen_golden(path):
    """B-30: 录制黄金轨迹 (确定性产物 sha256 表; 宿主侧无需 QEMU)。"""
    g = {}
    for v in range(1, 7):
        arg = "FujoOS BoxBridge v1" if v in (5,) else "fujobox-v0 payload"
        prod = run_verb(v, arg)
        g[str(v)] = {
            "verb": verb_name(v),
            "arg": arg,
            "len": len(prod),
            "sha256": hashlib.sha256(prod).hexdigest(),
        }
    json.dump(g, open(path, "w"), indent=1)
    print(f"[box] golden recorded: {path} ({len(g)} verbs)")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=PORT)
    ap.add_argument("--mon", type=int, default=MON_PORT)
    ap.add_argument("--mode", default="normal",
                    choices=["normal", "badart", "adapter", "fuzz"])
    ap.add_argument("--golden", default=None,
                    help="B-30 黄金轨迹 JSON (校验模式)")
    ap.add_argument("--golden-record", default=None,
                    help="B-30 录制黄金轨迹 JSON (生成后退出)")
    a = ap.parse_args()
    if a.golden_record:
        gen_golden(a.golden_record)
        return 0
    golden = json.load(open(a.golden)) if a.golden else None
    BoxServer(a.port, a.mon, a.mode, golden).run()


if __name__ == "__main__":
    main()
