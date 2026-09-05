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
    ("m126-abi", "ELF64 x appmgr", "sdk/build/m126_multi.initrd", "M126 RESULT: PASS", [], {}),
    ("m127-exec", "ELF64 x exec-mem", "sdk/linux/m127_exec.elf", "exec-child-ok", [], {}),
    # W17b: SMP AP 唤醒 —— 必须 -smp 2 (INIT+SIPI 序列, QEMU LAPIC 投递实测)
    ("m129-smp", "ELF64 x smp-ap", "sdk/linux/m129_smp.elf", "M129 RESULT: PASS",
     ["-smp", "2", "-accel", "tcg,thread=multi"], {}),
    # W16b: 自托管编译链 —— 注入: 写源码 -> tcc 编译 -> runfile 运行
    ("m128-tcc", "ELF64 x tcc-chain", "sdk/build/m128_tcc.initrd",
     "tcc-compiled hello from fujo",
     [],
     {"bootsleep": 12.0,
      "keys": ["m", "b", "u", "i", "l", "d", "ret", "wait:5",
               "r", "u", "n", "f", "i", "l", "e", "spc", "slash", "t", "m", "p", "slash", "h", "e", "l", "l", "o", "ret"]}),
    ("m130-audit", "ELF64 x unified-audit", "sdk/linux/m130_aud.elf", "M130 RESULT: PASS", [], {}),
    # W18: VFS 目录语义 (stat 类型 / open dir / getdents64; busybox ls 依据)
    ("m132-dirs", "ELF64 x dirs", "sdk/linux/m132_dirs.elf", "M132 RESULT: PASS", [], {}),
    # W20: 平台检测 (QEMU 证据链一致性; ICR 语义模式随平台)
    ("m133-plat", "ELF64 x platform", "sdk/linux/m133_plat.elf", "M133 RESULT: PASS", [], {}),
    # W20: AHCI (SATA) 驱动 —— q35 机器 (真机 SATA 路径)
    ("m134-ahci", "ELF64 x ahci", "sdk/linux/m134_ahci.elf", "M134 RESULT: PASS",
     ["-machine", "q35",
      "-drive", "if=none,id=hd,file=" + os.path.join(ROOT, "sdk", "ahci-mini.img") + ",format=raw",
      "-device", "ide-hd,drive=hd,bus=ide.0"], {}),
    # W20 p5: FJFS 卷经 AHCI 背板 (真机 SATA 持久化)
    ("m135-fs", "ELF64 x fjfs-ahci", "sdk/linux/m135_fs.elf", "M135 RESULT: PASS",
     ["-machine", "q35",
      "-drive", "if=none,id=hd,file=" + os.path.join(ROOT, "sdk", "ahci.img") + ",format=raw",
      "-device", "ide-hd,drive=hd,bus=ide.0"], {}),
    # W20 p6: 大内存拓扑 (>1GiB 可用区映射; -m 3072: QEMU 9.2 ≥4G 不提供
    # multiboot module (A/B 实证), 3072 是"高位映射 + module"窗口最大值;
    # PML4[1] (>4GiB) 验证记录 docs/79 (手动 -m 8192: 7167MiB mapped)
    ("m136-mem", "ELF64 x memtopo", "sdk/linux/m136_mem.elf", "M136 RESULT: PASS",
     ["-m", "3072"], {"bootsleep": 13.0}),
    # W20 p7: PCI 枚举完整化 (多功能设备; q35 SATA 31.2)
    ("m137-pci", "ELF64 x pcienum", "sdk/linux/m137_pci.elf", "M137 RESULT: PASS",
     ["-machine", "q35"], {}),
    # W21: UDP source clone (guest -> slirp 10.0.2.2:8077 -> host UDP server)
    ("m139-http", "ELF64 x udp-clone", "sdk/linux/m139_http.elf", "M139 RESULT: PASS",
     ["-netdev", "user,id=net0",
      "-device", "virtio-net-pci,netdev=net0,mac=52:54:00:12:34:57,disable-modern=on,disable-legacy=off"],
     {"udp_server": [8077, os.path.join(ROOT, "sdk", "network", "hello-clone.c")]}),
    # W22: 三引擎质量对照 (无模型 = 确定性 rules/auto 降级语义; 模型在线由 verify_ai 全量)
    # B20: n=100 goldset -> T4 link fallback (3x resend, spin-bound) needs ~150s
    ("m141-eval", "ELF64 x 3-engine-eval", "sdk/linux/m141_eval.elf", "M141 RESULT: PASS", [], {"timeout": 170}),
    # W22: 自监督反馈闭环 (anom 建议 -> 自动隔离 -> 内核验证位 -> 审计标签)
    ("m142-feedback", "ELF64 x ai-feedback", "sdk/linux/m142_feedback.elf", "M142 RESULT: PASS", [], {}),
    # W23: 蒸馏闭环自动化 (FJRU v2 19 条 -> novel 全命中 -> 零模型调用)
    ("m143-distill", "ELF64 x distill-feed", "sdk/linux/m143_distill_feed.elf", "M143 RESULT: PASS", [], {}),
    # W25: IO 预测所有权重判 (二阶马尔可夫基线 vs 模型)
    ("m145-io-own", "ELF64 x io-ownership", "sdk/linux/m145_io_own.elf", "M145 RESULT: PASS", [], {}),
    # W26: 五职责全自监督 (plan/nlc 动作后果验证 -> 审计 verified)
    ("m146-fullfb", "ELF64 x full-feedback", "sdk/linux/m146_full_fb.elf", "M146 RESULT: PASS", [], {}),
    # W27: 哨兵接管真实事件流 (ev_digest -> 分类 -> 自动隔离 -> 速率回落)
    ("m147-storm", "ELF64 x ev-storm", "sdk/linux/m147_storm.elf", "M147 RESULT: PASS", [], {"bootsleep": 10.0}),
    # W21: 自托管闭环 —— m139 clone 源码 (tmpfs+FJFS) -> mbuild /tmp/hello-clone.c
    #       (tcc-static 编译) -> runfile /tmp/hello (产物运行输出)
    ("m140-selfhost", "ELF64 x selfhost", "sdk/build/m140_self.initrd",
     "cloned-compiled hello from fujo",
     ["-netdev", "user,id=net0",
      "-device", "virtio-net-pci,netdev=net0,mac=52:54:00:12:34:57,disable-modern=on,disable-legacy=off"],
     {"udp_server": [8077, os.path.join(ROOT, "sdk", "network", "hello-clone.c")],
      "bootsleep": 10.0,
      "keys": ["o", "s", "spc", "r", "u", "n", "spc", "h", "e", "r", "m", "e", "s", "ret", "wait:0.5",
               "m", "b", "u", "i", "l", "d", "spc", "slash", "t", "m", "p", "slash", "h", "e", "l", "l", "o",
               "minus", "c", "l", "o", "n", "e", "dot", "c", "ret", "wait:3",
               "r", "u", "n", "f", "i", "l", "e", "spc", "slash", "t", "m", "p", "slash", "h", "e", "l", "l", "o", "ret"]}),
    # W18: 标准软件移植 —— 静态 busybox (musl) 原生命令在 FujoOS 内执行
    ("m131-bbx", "ELF64 x busybox-cmd", "sdk/busybox-musl", "m131-busybox-ok", [],
     {"keys": ["o", "s", "spc", "r", "u", "n", "spc", "b", "u", "s", "y", "b", "o", "x", "spc",
               "e", "c", "h", "o", "spc", "m", "1", "3", "1", "minus", "b", "u", "s", "y", "b", "o", "x", "minus", "o", "k", "ret"]}),
    # W30: autostart cmdline (mbi cmdline fujo.run=<demo> -> direct launch, 无 sendkey;
    #      GRUB/真机等效路径: mbi cmdline 由引导器交付)
    ("m148-autostart", "ELF64 x autostart", "sdk/linux/m142_feedback.elf", "M142 RESULT: PASS",
     [], {"append": "fujo.run=m142_feedback", "keys": []}),
    # W32: 信任自适应域 (质量台账 -> dom_admit -> 域宽=f(质量); zcode 框架)
    ("m149-trust", "ELF64 x trust-admit", "sdk/linux/m149_trust.elf", "M149 RESULT: PASS", [], {}),
    # B2: TCP 客户端数据面探针 (复现 W21 slirp 数据段 DROP; 双模式对照列)
    ("m150-tcpclient", "ELF64 x tcp-probe", "sdk/linux/m150_tcpclient.elf", "M150 RESULT: PASS",
     ["-netdev", "user,id=net0",
      "-device", "virtio-net-pci,netdev=net0,mac=52:54:00:12:34:57,disable-modern=on,disable-legacy=off"],
     {"tcp_server": [8021], "bootsleep": 10.0}),
    # W34: Windows 文件完美运行 —— 标准 Console API 集 (零 CRT PE32+)
    ("pe-m152", "PE32+ x win-console", "sdk/win/m152_win.exe", "W152 RESULT: PASS", [], {}),
    # W34: .run (FUJR) 容器内 Windows 载荷完美运行 (fujopack pack -e m152_win.exe)
    ("run-w152", "ELF(.run) x win32 console", "sdk/build/m152_win.run", "W152 RESULT: PASS", [], {}),
    # W34: .shell 脚本 (FUJR 容器 EMBED = #!fujoshell 文本; 内置解释器)
    ("run-w153", "ELF(.run) x fujo shell script", "sdk/build/m153_shell.run", "W153 RESULT: PASS", [], {}),
    # W35: 散件工厂 —— 公共域 sha256 工具 (fujo_libc 散件 + 原样源码拼装)
    # 本波: 宿主编译 (WSL gcc -nostdlib) -> FujoOS 原生 ELF 运行; 向量验证
    # B类后续: 内核内 tcc 编译大字源 (GP at tcc 0x49f630, 与大小无关, docs/106)
    ("scatter", "ELF64 x scatter-tool", "sdk/build/sha256tool.elf",
     "SFACTORY RESULT: PASS", [], {}),
]

MON_PORT = 4568
SER_PORT = 4001
KEYS = ["o", "s", "spc", "r", "u", "n", "spc", "h", "e", "r", "m", "e", "s", "ret"]


def qemu():
    return shutil.which("qemu-system-x86_64")


def run_case(kernel, case, timeout_s, accel="tcg"):
    name, label, rel, needle = case[:4]
    extra = list(case[4]) if len(case) > 4 else []
    opts = case[5] if len(case) > 5 else {}
    timeout_s = float(opts.get("timeout", timeout_s))
    # W29: WHPX 对照 —— 关闭内核中断芯片 (QEMU WHPX 默认 kernel-irqchip=on 只走
    # APIC 注入, legacy 8259 直连路径失效 -> m1 等 PIT tick 死锁; off 后设备模型
    # 模拟 PIC/PIT, 与 TCG/真机同构)
    if accel == "whpx":
        if "-machine" in extra:
            idx = extra.index("-machine")
            extra[idx + 1] = extra[idx + 1] + ",kernel-irqchip=off"
        else:
            extra = ["-machine", "kernel-irqchip=off"] + extra
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
    # W21: host 侧 UDP source server (m139: guest UDP GET-SOURCE -> host 回源码)
    if opts.get("udp_server"):
        hport = int(opts["udp_server"][0])
        hfile = opts["udp_server"][1]
        hbody = open(hfile, "rb").read()
        hstop = [False]

        def hsrv():
            s = sk.socket(sk.AF_INET, sk.SOCK_DGRAM)
            s.setsockopt(sk.SOL_SOCKET, sk.SO_REUSEADDR, 1)
            s.bind(("127.0.0.1", hport))
            s.settimeout(0.5)
            while not hstop[0]:
                try:
                    d, a = s.recvfrom(2048)
                except sk.timeout:
                    continue
                except OSError:
                    break
                print(f"udpsrv: req {len(d)}B from {a}", flush=True)
                s.sendto(hbody, a)
        threading.Thread(target=hsrv, daemon=True).start()
    # B2 (W21 followup): host 侧 TCP echo server (m150: guest -> slirp 10.0.2.2:port)
    if opts.get("tcp_server"):
        tport = int(opts["tcp_server"][0])
        tstop_t = [False]

        def tsrv():
            s = sk.socket(sk.AF_INET, sk.SOCK_STREAM)
            s.setsockopt(sk.SOL_SOCKET, sk.SO_REUSEADDR, 1)
            s.bind(("127.0.0.1", tport))
            s.listen(2)
            while not tstop_t[0]:
                s.settimeout(0.5)
                try:
                    conn, addr = s.accept()
                except sk.timeout:
                    continue
                except OSError:
                    break
                try:
                    while True:
                        d = conn.recv(2048)
                        if not d:
                            break
                        print(f"tcpsrv: rx {len(d)}B", flush=True)
                        conn.sendall(d)
                except Exception:
                    pass
                finally:
                    conn.close()
        threading.Thread(target=tsrv, daemon=True).start()
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
        qemu(), "-m", "256M", "-accel", accel, "-kernel", kernel, "-initrd", initrd,
        "-serial", f"file:{log}",
        "-serial", f"tcp:127.0.0.1:{SER_PORT},server=on,wait=off",
        "-monitor", f"telnet:127.0.0.1:{MON_PORT},server,nowait",
        "-display", "none", "-no-reboot",
    ] + (["-append", opts.get("append")] if opts.get("append") else []) + extra)
    time.sleep(float(opts.get("bootsleep", 9.0)))
    try:
        s = socket.create_connection(("127.0.0.1", MON_PORT), timeout=3)
        f = s.makefile("w")
        keys = opts.get("keys", KEYS)
        t0 = time.time()
        for ki, k in enumerate(keys):
            if isinstance(k, str) and k.startswith("wait:"):
                time.sleep(float(k[5:]))
                continue
            if isinstance(k, str) and k == "reconnect":
                try:
                    s.close()
                except Exception:
                    pass
                s = socket.create_connection(("127.0.0.1", MON_PORT), timeout=3)
                f = s.makefile("w")
                continue
            f.write(f"sendkey {k}\n")
            f.flush()
            time.sleep(0.25)
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
        # W15: 命中 needle 即提前杀 (demo 完成后内核回 shell, QEMU 不退出 ——
        # 原逻辑每用例空等满 timeout, 17 用例 ~60min; 现在 ~25s/用例)
        if needle and os.path.exists(log):
            try:
                with open(log, errors="replace") as lf:
                    if needle in lf.read():
                        break
            except Exception:
                pass
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
    ap.add_argument("--accel", default="tcg", help="QEMU accel: tcg (default) | whpx (W29 second-execution-mode contrast)")
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
        st, err, logtxt = run_case(a.kernel, case, a.timeout, a.accel)
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
