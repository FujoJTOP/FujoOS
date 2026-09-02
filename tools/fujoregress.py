#!/usr/bin/env python3
"""fujoregress — 兼容矩阵自动化回归 (M34)

三格式 × 三子系统 × 附加层 逐用例: QEMU 启动(内核+initrd) -> 自动注入
`os run hermes` -> 抓串口日志 -> 断言关键字。矩阵:

  格式    子系统        用例               断言
  ELF     linuxsubsys   sdk/linux/m30_linux.elf    M30 RESULT: PASS
  ELF     linuxsubsys   sdk/linux/m33_trace.elf    M33 RESULT: PASS
  ELF.run fujopack      sdk/build/m31_res.run      M31 RESULT: PASS
  ELF.multi fujorun     sdk/build/m32_multi.initrd M32 RESULT: PASS
  Mach-O  darwinsubsys  sdk/mac/m29_darwin.macho   M29 RESULT: PASS
  PE32+   winsubsys(M3) sdk/win/hello_win.exe      M3 verified
  PE32+   winsubsys(M26)sdk/win/m26_win.exe        M26 RESULT: PASS
  PE32+   winsubsys(M27)sdk/win/m27_mingw.exe      M27 RESULT: PASS
  PE32+   winsubsys(M30)sdk/win/m30_win.exe        M30 RESULT: PASS

用法: python tools/fujoregress.py [-k kernel.bin] [--only IX] [--timeout N]
"""
import argparse
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
KERNEL = os.path.join(ROOT, "kernel", "fujo-kernel.bin")

CASES = [
    ("elf-linux", "ELF64 x linuxsubsys", "sdk/linux/m30_linux.elf", "M30 RESULT: PASS"),
    ("elf-linux2", "ELF64 x linuxsubsys", "sdk/linux/m33_trace.elf", "M33 RESULT: PASS"),
    ("run-fujopack", "ELF(.run) x fujopack", "sdk/build/m31_res.run", "M31 RESULT: PASS"),
    ("multi-fujorun", "ELF(+lib) x fujorun", "sdk/build/m32_multi.initrd", "M32 RESULT: PASS"),
    ("macho-darwin", "Mach-O x darwinsubsys", "sdk/mac/m29_darwin.macho", "M29 RESULT: PASS"),
    ("pe-m3", "PE32+ x winsubsys", "sdk/win/hello_win.exe", "M3 verified"),
    ("pe-m26", "PE32+ x winsubsys", "sdk/win/m26_win.exe", "M26 RESULT: PASS"),
    ("pe-m27", "PE32+ x winsubsys", "sdk/win/m27_mingw.exe", "M27 RESULT: PASS"),
    ("pe-m30", "PE32+ x winsubsys", "sdk/win/m30_win.exe", "M30 RESULT: PASS"),
    ("r1-invariants", "ELF64 x aximatic", "sdk/linux/m119_inv.elf", "M119 RESULT: PASS"),
    ("m116-domain", "ELF64 x explosion", "sdk/linux/m116_dom.elf", "M116 RESULT: PASS"),
    ("m120-distill", "ELF64 x distill", "sdk/linux/m120_distill.elf", "M120 RESULT: PASS"),
    ("m121-isol", "ELF64 x aspace", "sdk/linux/m121_isol.elf", "M121 RESULT: PASS"),
    ("m122-dev", "ELF64 x modeldev", "sdk/linux/m122_dev.elf", "M122 RESULT: PASS"),
    ("m123-vblk", "ELF64 x virtio", "sdk/linux/m123_vblk.elf", "M123 RESULT: PASS",
     ["-drive", f"if=none,id=vblk,file={os.path.join(ROOT, 'sdk', 'vblk.img')},format=raw",
      "-device", "virtio-blk-pci,drive=vblk,disable-modern=on,disable-legacy=off,queue-size=16"]),
    ("m124-net", "ELF64 x udp-echo", "sdk/linux/m124_net.elf", "M124 RESULT: PASS",
     ["-netdev", "user,id=net0",
      "-device", "virtio-net-pci,netdev=net0,mac=52:54:00:12:34:57,disable-modern=on,disable-legacy=off"],
     {"udp_echo": True, "port": 7777}),
    ("m125-tcp", "ELF64 x tcp-echo", "sdk/linux/m125_tcp.elf", "M125 RESULT: PASS",
     ["-netdev", "user,id=net0,hostfwd=tcp:127.0.0.1:18080-:8080",
      "-device", "virtio-net-pci,netdev=net0,mac=52:54:00:12:34:57,disable-modern=on,disable-legacy=off"],
     {"tcp_client": [18080, b"fujo-tcp-echo-payload-64x!", 12.0]}),
]

MON_PORT = 4568
SER_PORT = 4001
KEYS = ["o", "s", "spc", "r", "u", "n", "spc", "h", "e", "r", "m", "e", "s", "ret"]


def qemu():
    return shutil.which("qemu-system-x86_64")


def run_case(kernel, case, timeout_s):
    name, label, rel, needle = case[:4]
    extra = list(case[4]) if len(case) > 4 else []
    opts = case[5] if len(case) > 5 else {}
    # host 侧 UDP echo (m124: QEMU slirp 10.0.2.2:port -> 127.0.0.1:port)
    import socket as sk
    import threading
    echo_stop = [False]
    if opts.get("udp_echo"):
        eport = int(opts.get("port", 7777))

        def srv():
            s = sk.socket(sk.AF_INET, sk.SOCK_DGRAM)
            s.bind(("127.0.0.1", eport))
            s.settimeout(0.2)
            while not echo_stop[0]:
                try:
                    d, a = s.recvfrom(2048)
                    print(f"echo: rx {len(d)}B", flush=True)
                    s.sendto(d, a)
                except sk.timeout:
                    pass
        threading.Thread(target=srv, daemon=True).start()
    # host 侧 TCP client (m125: 经 slirp hostfwd -> guest:8080)
    if opts.get("tcp_client"):
        cport = int(opts["tcp_client"][0])
        pay = opts["tcp_client"][1]
        cdelay = float(opts["tcp_client"][2]) if len(opts["tcp_client"]) > 2 else 11.0

        def cli():
            time.sleep(cdelay)
            try:
                s = sk.create_connection(("127.0.0.1", cport), timeout=6)
                s.sendall(pay)
                r = s.recv(2048)
                ok = r == pay
                print(f"tcpclient: rx {len(r)}B ok={ok}", flush=True)
                s.close()
            except Exception as e:
                print(f"tcpclient: fail {e}", flush=True)
        threading.Thread(target=cli, daemon=True).start()
    initrd = os.path.join(ROOT, rel)
    if not os.path.exists(initrd):
        return ("MISS", f"initrd not found: {initrd}", "")
    # 每用例前清扫残留 qemu (端口独占; M39 起内核变大, 旧实例退出慢)
    subprocess.run(["taskkill", "/F", "/IM", "qemu-system-x86_64.exe"],
                   capture_output=True)
    time.sleep(0.8)
    tmpd = tempfile.mkdtemp(prefix="fujoregress-")
    log = os.path.join(tmpd, "qemu.log")
    # 独占端口: 用例间释放
    p = subprocess.Popen([
        qemu(), "-m", "256M", "-kernel", kernel, "-initrd", initrd,
        "-serial", f"file:{log}",
        "-serial", f"tcp:127.0.0.1:{SER_PORT},server=on,wait=off",
        "-monitor", f"telnet:127.0.0.1:{MON_PORT},server,nowait",
        "-display", "none", "-no-reboot",
    ] + extra)
    time.sleep(9.0)
    try:
        s = socket.create_connection(("127.0.0.1", MON_PORT), timeout=3)
        f = s.makefile("w")
        for k in KEYS:
            f.write(f"sendkey {k}\n")
            f.flush()
            time.sleep(0.10)
        s.close()
    except OSError:
        pass
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        try:
            if p.poll() is not None:
                break
        except Exception:
            break
        time.sleep(0.5)
    time.sleep(1.0)
    try:
        p.kill()
    except Exception:
        pass
    echo_stop[0] = True
    log_txt = ""
    if os.path.exists(log):
        log_txt = open(log, errors="replace").read()
    if needle in log_txt:
        return ("PASS", "", log_txt)
    tail = "\n".join(log_txt.splitlines()[-8:])
    return ("FAIL", f"needle '{needle}' not found", tail)


def main():
    ap = argparse.ArgumentParser(prog="fujoregress")
    ap.add_argument("-k", "--kernel", default=KERNEL)
    ap.add_argument("--only", type=int, default=None)
    ap.add_argument("--timeout", type=float, default=14.0)
    ap.add_argument("--json", default=None)
    a = ap.parse_args()
    if not os.path.exists(a.kernel):
        print(f"kernel not found: {a.kernel}", file=sys.stderr)
        return 1
    results = []
    for i, case in enumerate(CASES):
        if a.only is not None and i != a.only:
            continue
        label = f"[{i}] {case[0]:16s} {case[1]:28s}"
        print(f":: {label} ...", flush=True)
        st, err, logtxt = run_case(a.kernel, case, a.timeout)
        print(f":: {label} {st} {err}")
        if st != "PASS":
            print(logtxt)
        results.append({"case": case[0], "label": case[1], "status": st, "needle": case[3]})
    ok = sum(1 for r in results if r["status"] == "PASS")
    print("-" * 60)
    print(f"matrix: {ok}/{len(results)} PASS")
    if a.json:
        json.dump(results, open(a.json, "w"), indent=1)
    return 0 if ok == len(results) else 1


if __name__ == "__main__":
    sys.exit(main())
