#!/bin/bash
# W33 B4: KVM 全量矩阵 (无 host 服务依赖的 14 用例; 每 demo 55s)
# 用法: bash tools/kvm-matrix.sh
ROOT=/mnt/d/Dev/FujoOS
KERNEL=$ROOT/kernel/fujo-kernel.bin
OUT=/mnt/c/Users/hooya/AppData/Local/Temp/kvm-matrix
mkdir -p "$OUT"

run_one() {
  local demo=$1; shift
  local log="$OUT/$demo.log"
  rm -f "$log"
  qemu-system-x86_64 -m 256M -enable-kvm \
    -kernel "$KERNEL" \
    -initrd "$ROOT/sdk/linux/$demo.elf" \
    -append "fujo.run=$demo" \
    -serial "file:$log" \
    -display none -no-reboot "$@" &
  local pid=$!
  sleep 55
  kill $pid 2>/dev/null
  if grep -q "RESULT: PASS" "$log" 2>/dev/null; then
    echo "PASS $demo"
  else
    echo "FAIL $demo (see $log)"
  fi
}

VBLK="$ROOT/sdk/vblk.img"
AHCI="$ROOT/sdk/ahci.img"

run_one m116_dom
run_one m119_inv
run_one m120_distill
run_one m121_isol
run_one m122_dev
run_one m123_vblk -drive if=none,id=vblk,file=$VBLK,format=raw \
  -device virtio-blk-pci,drive=vblk,disable-modern=on,disable-legacy=off,queue-size=16
run_one m126_abi
run_one m127_exec
run_one m130_aud
run_one m132_dirs
run_one m133_plat
run_one m134_ahci -machine q35 \
  -drive if=none,id=hd,file=$ROOT/sdk/ahci-mini.img,format=raw \
  -device ide-hd,drive=hd,bus=ide.0
run_one m135_fs -machine q35 \
  -drive if=none,id=hd,file=$AHCI,format=raw \
  -device ide-hd,drive=hd,bus=ide.0
run_one m136_mem -m 3072
run_one m137_pci -machine q35
echo "matrix done"
