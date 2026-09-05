#!/bin/bash
# W31: WSL2 KVM 对照验收 (硬件虚拟化执行; 无 TCG 解释)
# 用法: bash tools/kvm-run.sh <demo> <logfile>
DEMO=$1
LOG=$2
rm -f "$LOG"
qemu-system-x86_64 -m 256M -enable-kvm \
  -kernel /mnt/d/Dev/FujoOS/kernel/fujo-kernel.bin \
  -initrd "/mnt/d/Dev/FujoOS/sdk/linux/$DEMO.elf" \
  -append "fujo.run=$DEMO" \
  -serial "file:$LOG" \
  -display none -no-reboot &
QPID=$!
sleep 55
kill $QPID 2>/dev/null
wait $QPID 2>/dev/null
echo "done"
