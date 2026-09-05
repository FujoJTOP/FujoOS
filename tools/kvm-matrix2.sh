#!/bin/bash
ROOT=/mnt/d/Dev/FujoOS
KERNEL=$ROOT/kernel/fujo-kernel.bin
OUT=/mnt/c/Users/hooya/AppData/Local/Temp/kvm-matrix
mkdir -p "$OUT"
run_one() {
  local demo=$1; shift
  local log="$OUT/$demo.log"
  rm -f "$log"
  qemu-system-x86_64 -m 256M -enable-kvm -kernel "$KERNEL" \
    -initrd "$ROOT/sdk/linux/$demo.elf" -append "fujo.run=$demo" \
    -serial "file:$log" -display none -no-reboot "$@" &
  local pid=$!
  sleep 55
  kill $pid 2>/dev/null
  if grep -qE "RESULT: PASS|exec-child-ok" "$log" 2>/dev/null; then echo "PASS $demo"; else echo "FAIL $demo"; fi
}
run_one m133_plat
run_one m134_ahci -machine q35 -drive if=none,id=hd,file=$ROOT/sdk/ahci-mini.img,format=raw -device ide-hd,drive=hd,bus=ide.0
run_one m135_fs -machine q35 -drive if=none,id=hd,file=$ROOT/sdk/ahci.img,format=raw -device ide-hd,drive=hd,bus=ide.0
run_one m136_mem -m 3072
run_one m137_pci -machine q35
echo done
