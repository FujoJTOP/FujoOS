#!/bin/bash
# W31: WSL2 KVM 验收脚本 (m142 autostart)
LOG=/mnt/c/Users/hooya/AppData/Local/Temp/kvm-test3.log
rm -f "$LOG"
qemu-system-x86_64 -m 256M -enable-kvm \
  -kernel /mnt/d/Dev/FujoOS/kernel/fujo-kernel.bin \
  -initrd /mnt/d/Dev/FujoOS/sdk/linux/m142_feedback.elf \
  -append "fujo.run=m142_feedback" \
  -serial "file:$LOG" \
  -display none -no-reboot &
QPID=$!
sleep 40
kill $QPID 2>/dev/null
echo "done"
