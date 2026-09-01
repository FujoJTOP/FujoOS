# 46 — 真机显示/键盘/存储适配 (M97, 参考机: QEMU)

状态: ✅ 完成。验收: `M97 RESULT: PASS` + FJFS 两阶段持久化 PASS。

## 1. 接口 (hw.rs)

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x8601 | hw_disp(ptr) | (fbw, fbh, lfb_ok, kbd_irqs) |
| 0x8602 | hw_storage(ptr) | (ata, lba48, fs_ok, files) |

- kbd_irqs: kbd.rs 中断挂点 (hw::kbd_note) — 每 IRQ1 计数;
- lba48: ata.rs IDENTIFY words[83|86] bit10 (新增 lba48_capable);
- fs_ok/files: fjfs VOLUME_OK / 根目录条目计数。

## 2. 适配面

| 子系统 | 路径 | 备注 |
|--------|------|------|
| 显示 | VBE 1024x768x32 + LFB (M47) | fb_w/h 8:0c00/0300 |
| 键盘 | PS/2 IRQ1 scancode set1 | IRQ 计数可观测 |
| 存储 | ATA PIO + FJFS (M16) | IDENTIFY/lba48/persist |

## 3. 实测

```
m97: fb=00000400x00000300 kbd_irq=0000001d ata=00000001 fs=00000000
m97: M97 RESULT: PASS
```

## 4. FJFS 两阶段持久化 (存储适配验收)

```
# qemu -m 256M -kernel ... -initrd sdk/user/disk_fs.elf \
#     -drive file=.m97disk.img,format=raw,if=ide
阶段1 (新镜像): FJFS persistent data #1 → 写盘 → flush ok
阶段2 (同镜像重boot): 读回 → FJFS persistent data #1 / seen-boot2
  → 数据跨重启在盘上 (FJFS 持久)
```

## 5. 后续

- M98 live 镜像 = 内核 + 磁盘卷 (rootfs) + 安装器;
- 真机适配: PCI 设备路径 (M96 枚举) + 高内存 ACPI 表映射 (M96 guard)。
