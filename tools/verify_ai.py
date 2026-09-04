#!/usr/bin/env python3
"""verify_ai.py — AI For Next 里程碑无头验证 (通用: --demo/--needle/--model)

QEMU(COM1=file 日志 / COM2=tcp:4000 模型链路 / monitor=4568) + shm 模型服务
(服务端经 monitor 注入 boot keys) -> 等待 needle。
"""
import argparse
import os
import shutil
import subprocess
import sys
import tempfile
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
KERNEL = os.path.join(ROOT, "kernel", "fujo-kernel.bin")
MON_PORT = 4568
LINK_PORT = 4000


def kill_peers():
    subprocess.run(["taskkill", "/F", "/IM", "qemu-system-x86_64.exe"], capture_output=True)
    try:
        out = subprocess.run(
            ["wmic", "process", "where", "name='python.exe'", "get", "ProcessId,CommandLine", "/format:csv"],
            capture_output=True, text=True).stdout
    except Exception:
        out = ""
    for ln in out.splitlines():
        if "qwen_model_server.py" in ln:
            parts = ln.strip().split(",")
            if parts and parts[-1].isdigit():
                try:
                    subprocess.run(["taskkill", "/F", "/PID", parts[-1]], capture_output=True)
                except Exception:
                    pass


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--demo", default="m112_ai")
    ap.add_argument("--needle", default="M112 RESULT: PASS")
    ap.add_argument("--model", default="qwen2.5:0.5b")
    ap.add_argument("--timeout", type=float, default=420.0)
    ap.add_argument("--boot-wait", type=float, default=9.0)
    ap.add_argument("--boot-keys", default="o s spc r u n spc h e r m e s ret")
    ap.add_argument("--evil", action="store_true", help="W24: adversarial model replies (FUJO_EVIL=1)")
    ap.add_argument("--accel", default="tcg", help="QEMU accel (W29 contrast: tcg | whpx)")
    a = ap.parse_args()

    kill_peers()
    time.sleep(1.0)
    tmpd = tempfile.mkdtemp(prefix="ai-verify-")
    log = os.path.join(tmpd, "qemu.log")
    initrd = os.path.join(ROOT, "sdk", "linux", f"{a.demo}.elf")

    qemu = shutil.which("qemu-system-x86_64")
    machine = []
    if a.accel == "whpx":
        # W29: WHPX 对照 —— legacy 8259 直连需 kernel-irqchip=off (详见 docs/92)
        machine = ["-machine", "kernel-irqchip=off"]
    p = subprocess.Popen([
        qemu, "-m", "256M", "-accel", a.accel, *machine, "-kernel", KERNEL, "-initrd", initrd,
        "-serial", f"file:{log}",
        "-serial", f"tcp:127.0.0.1:{LINK_PORT},server=on,wait=off",
        "-monitor", f"telnet:127.0.0.1:{MON_PORT},server,nowait",
        "-display", "none", "-no-reboot",
    ])
    env = dict(os.environ,
               FUJO_MODEL=a.model, FUJO_MON_PORT=str(MON_PORT), FUJO_LINK_PORT=str(LINK_PORT),
               FUJO_BOOT_KEYS=a.boot_keys, FUJO_BOOT_WAIT=str(a.boot_wait))
    if a.evil:
        env["FUJO_EVIL"] = "1"
    srvlog = os.path.join(tmpd, "server.log")
    srv_out = open(srvlog, "w")
    srv = subprocess.Popen([sys.executable, os.path.join(ROOT, "tools", "qwen_model_server.py")], env=env,
                           stdout=srv_out, stderr=subprocess.STDOUT, text=True)

    print(f"[verify] booted demo={a.demo} model={a.model} (log={log})", flush=True)
    deadline = time.time() + a.timeout
    result = ""
    while time.time() < deadline:
        try:
            if os.path.exists(log):
                txt = open(log, errors="replace").read()
                if a.needle in txt:
                    result = "PASS"
                    break
                if a.needle.replace(": PASS", ": FAIL") in txt:
                    result = "FAIL"
                    break
        except OSError:
            pass
        time.sleep(2.0)
    if not result:
        result = "TIMEOUT"
    try:
        p.kill()
    except Exception:
        pass
    time.sleep(1.0)
    try:
        if srv.poll() is None:
            srv.kill()
    except Exception:
        pass
    try:
        srv_out.close()
    except Exception:
        pass
    out = ""
    try:
        out = open(log, errors="replace").read()
    except OSError:
        pass
    print("=" * 60)
    print(f"[verify] result: {result}")
    print("[verify] serial log tail:")
    print("\n".join(out.splitlines()[-45:]))
    print("[verify] server log tail:")
    try:
        slog = open(srvlog, errors="replace").read()
        print("\n".join(slog.splitlines()[-12:]))
    except OSError:
        pass
    return 0 if result == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
