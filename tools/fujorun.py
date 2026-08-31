#!/usr/bin/env python3
"""fujorun — FujoOS 模块运行器 (M32): 多模块 initrd + QEMU 启动

多模块镜像格式 (BootMulti v1, 内核 fujorun_multi 解析):
  [0..7]  "FUJOMULT"
  [8..15] count u64
  [16..16+32*count)  count × (off u64, len u64, name[16])   (数据区 8 对齐)
  [..]    模块数据 (模块 0 = 可执行体, 模块 1.. = 库/资源)

用法:
  fujorun.py pack -i main.run --lib lib.run ... -o multi.bin
  fujorun.py run -k kernel.bin -i main.run --lib lib.run ...   (QEMU 启动)
"""
import argparse
import subprocess
import sys
import tempfile
import os


def pack(items) -> bytes:
    count = len(items)
    hdr = b"FUJOMULT" + count.to_bytes(8, "little")
    entries = bytearray()
    payload = bytearray()
    off = (16 + 32 * count + 7) & ~7
    for (name, data) in items:
        pad = (-len(payload)) & 7
        payload.extend(b"\0" * pad)
        off += pad
        nb = name.encode()[:15]
        entries.extend(off.to_bytes(8, "little") + len(data).to_bytes(8, "little") + nb + b"\0" * (16 - len(nb)))
        payload.extend(data)
        off += len(data)
    return hdr + bytes(entries) + bytes(payload)


def qemu_path():
    import shutil
    return shutil.which("qemu-system-x86_64") or r"C:\Program Files\qemu\qemu-system-x86_64.exe"


def main():
    ap = argparse.ArgumentParser(prog="fujorun")
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("pack")
    p.add_argument("-i", "--input", required=True, help="主模块 (可执行 .run/.elf/.exe/.macho)")
    p.add_argument("--lib", action="append", default=[], help="附加库/资源模块 file")
    p.add_argument("-o", "--out", required=True)
    p2 = sub.add_parser("run")
    p2.add_argument("-k", "--kernel", required=True)
    p2.add_argument("-i", "--input", required=True)
    p2.add_argument("--lib", action="append", default=[])
    p2.add_argument("--mem", default="256M")
    p2.add_argument("--log", default=None)
    p2.add_argument("--keys", default=None, help="启动后注入的键盘序列 (空格分隔)")
    a = ap.parse_args()

    with open(a.input, "rb") as f:
        main_blob = f.read()
    libs = []
    for pth in a.lib:
        with open(pth, "rb") as f:
            libs.append((os.path.basename(pth), f.read()))

    if a.cmd == "pack":
        img = pack([("main", main_blob)] + libs)
        with open(a.out, "wb") as f:
            f.write(img)
        print(f"fujorun: wrote {a.out} ({len(img)} bytes, {1 + len(libs)} modules)")
        return 0

    if a.cmd == "run":
        img = pack([("main", main_blob)] + libs)
        tmpd = tempfile.mkdtemp(prefix="fujorun-")
        img_path = os.path.join(tmpd, "multi.initrd")
        with open(img_path, "wb") as f:
            f.write(img)
        log_path = a.log or os.path.join(tmpd, "qemu.log")
        cmd = [
            qemu_path(), "-m", a.mem, "-kernel", a.kernel,
            "-initrd", img_path,
            "-serial", f"file:{log_path}",
            "-serial", "tcp:127.0.0.1:4001,server=on,wait=off",
            "-monitor", "telnet:127.0.0.1:4568,server,nowait",
            "-display", "none", "-no-reboot",
        ]
        print("fujorun: qemu " + " ".join(cmd[2:5]) + " log=" + log_path)
        if a.keys:
            import socket
            proc = subprocess.Popen(cmd)
            import time
            time.sleep(8)
            try:
                s = socket.create_connection(("127.0.0.1", 4568), timeout=3)
                f = s.makefile("w")
                for k in a.keys.split():
                    f.write(f"sendkey {k}\n")
                    f.flush()
                    time.sleep(0.06)
                s.close()
            except OSError:
                pass
            proc.wait()
            print(f"fujorun: exit={proc.returncode} log=\n{open(log_path, errors='replace').read()[-3000:]}")
        return 0
    return 1


if __name__ == "__main__":
    sys.exit(main())
