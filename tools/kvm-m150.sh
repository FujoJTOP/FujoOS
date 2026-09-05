#!/bin/bash
# B2: KVM 下 TCP 客户端数据面探针 (对照 TCG 的 DATA_SEGMENT=DROP)
LOG=/mnt/c/Users/hooya/AppData/Local/Temp/kvm-m150.log
rm -f "$LOG"
# host echo server (slirp 10.0.2.2 -> 127.0.0.1:8021)
python3 - <<'EOF' &
import socket, threading
s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind(("127.0.0.1", 8021)); s.listen(2)
def echo(c):
    try:
        while True:
            d = c.recv(2048)
            if not d: break
            print("tcpsrv: rx", len(d), flush=True)
            c.sendall(d)
    except Exception: pass
    c.close()
while True:
    s.settimeout(0.5)
    try:
        conn, addr = s.accept()
    except socket.timeout:
        continue
    except OSError:
        break
    threading.Thread(target=echo, args=(conn,), daemon=True).start()
EOF
SRVPID=$!
sleep 1
qemu-system-x86_64 -m 256M -enable-kvm \
  -kernel /mnt/d/Dev/FujoOS/kernel/fujo-kernel.bin \
  -initrd /mnt/d/Dev/FujoOS/sdk/linux/m150_tcpclient.elf \
  -append "fujo.run=m150_tcpclient" \
  -netdev user,id=net0 \
  -device virtio-net-pci,netdev=net0,mac=52:54:00:12:34:57,disable-modern=on,disable-legacy=off \
  -serial "file:$LOG" \
  -display none -no-reboot &
QPID=$!
sleep 45
kill $QPID 2>/dev/null
kill $SRVPID 2>/dev/null
echo done
