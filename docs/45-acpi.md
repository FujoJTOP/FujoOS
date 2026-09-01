# 45 — 真机引导最小集 (M96: ACPI/PCI 表)

状态: ✅ 完成。验收: QEMU 串口 `M96 RESULT: PASS`, demo `sdk/linux/m96_acpi.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x8501 | acpi_info(ptr) | (rsdp_found, rev, table_count, pci_devs) |
| 0x8502 | acpi_dump(ptr, cap) | 摘要文本 |
| 0x8503 | pci_scan(ptr) | PCI 条目转储 (≤24) |

## 2. 探测

- RSDP: 0xE0000..0x100000 16 对齐搜索 magic `RSD PTR `
  (字节: R S D ' ' P T R ' ') — 踩坑: 初版把 P 当 idx5 (实际 +4);
- 表计数: XSDT (rev≥2) / RSDT 采样 — **guard**: 表体指针 >64MiB
  (QEMU 0xFFExxx) 时 boot 页表 (恒等 0..64MiB) 未映射 → 返回 0
  (记录为已知限制; 真机 ASCII 后映射高内存再遍历);
- PCI: CF8/CFC, bus 0..2 × slot 0..31 × func0, 非 FFFF/0 记录
  {vid16|did16<<16|bus<<32|slot<<40} — 启动时 scan_all
  (`pci : scanned 4 devices`)。

## 3. 实测 (m96_acpi.elf)

```
m96: rsdp=00000001 rev=00000000 tabs=00000000 pci=00000004
     vid0=00008086 did0=00001237
m96: M96 RESULT: PASS
```

- RSDP 找到 (SeaBIOS ACPI1.0 rev=0);
- 4 个 PCI 设备 (host bridge 8086:1237 为首);
- 表体计数 0 (高地址未映射限制, 文档化)。

## 4. 下一步 (M97)

- 真机显示/键盘/存储: ACPI PCI 设备路径 (VGA/IDE) 以本探测为
  起点; 高内存表映射列入内核扩展。
