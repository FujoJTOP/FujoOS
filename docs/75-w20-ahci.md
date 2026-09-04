# 75 · W20 p4 — AHCI (SATA) 驱动 (ICH9/q35; m134)

> 里程碑: W20 p4 (脱 QEMU 专属) · 上游: docs/74 审计表 #5b
> 一句话: **真机 SATA 主盘路径 —— PCI AHCI HBA 探测 (Bochs 无 → 真机 QEMU q35
> ich9-ahci 参考机) + BAR5 映射 + 单槽命令引擎 (0x25/0x35 DMA), 读/写/回读
> 数据级 PASS (m134; QEMU q35 机器, 真机 SATA 同路径)。**

## 1. 交付

| 部件 | 说明 |
|---|---|
| `kernel/src/ahci.rs` (新) | PCI 0x8086:0x2922 (ICH9 SATA) 查找 → 命令寄存器 0x7 (铁律 15) → BAR5 (0x24) → `map_phys_identity` → GHC.HR 复位 + AE → 端口 PxCMD 停/CLB/FB → FRE\|ST → 签名 0x101 (ATA) → 槽 0 DMA 引擎 |
| 原语 | 0x8E01 ahci_read(lba,buf) / 0x8E02 ahci_write(lba,buf) / 0x8E03 ahci_info(ptr) |
| `mem.rs::virt_to_phys` | 4 级页表行走 (M121 任务页表: 用户虚拟≠物理; DMA 必须 guest-physical) |
| `acpi.rs::pci_find` 修复 | 原 func 循环在 func0 不匹配时 break 整个 slot (Q35 AHCI=slot31.func2 永远找不到); 现逐 func 独立遍历 |
| `ata.rs` identify 空通道修复 | ident[0]==0 → 无盘 (QEMU 空通道垃圾曾误报 present) |
| `sdk/linux/m134_ahci.c` | T1 present; T2 读有效; T3 写 0xAB 回读; T4 lba_cap; **幂等 (结尾恢复参考盘)** |
| `tools/mk_ahci.py` | 参考盘 (8 扇区, i 模式); build-samples 注册 |
| `tools/fujoregress.py` | m134-ahci 用例: **-machine q35** + ide-hd bus=ide.0 |

**实测** (QEMU 9.2 q35):
```
m134: T1 info -> present ok
m134: T2 read sector 7 -> data0=abababab read ok
m134: T3 write/readback -> rw ok
m134: T4 lba_cap -> cap=4096 ok
m134: M134 RESULT: PASS
```

## 2. 根因链 (QEMU 9.2 AHCI "命令不完成" 真相)

**症状**: PxCI 置位后永不完成 (tfd=0x130: ABRT\|IDNF; PxIS=0x1 DHR)。

| # | 根因 | 证据 (QEMU trace + 源码 `hw/ide/ahci.c` / `ahci-internal.h` v9.2) |
|---|------|------|
| 1 | **FIS_REG_H2D command 在 offset 2** (非 3; 3 是 features) | `ide_bus_exec_cmd(..., cmd_fis[2])`; 我们把 cmd 写 offsets 3 → QEMU 当 features, feature=0x25 进特征, 莫名命令 |
| 2 | **count 在 FIS [12]/[13]** (非 [11]/[12]; [11]=hob_feature) | `nsector = (fis[13]<<8) \| fis[12]` |
| 3 | **QEMU AHCICmdHdr 布局与 AHCI 1.3 教程不同**: `opts@0x00` (bits4:0=CFL, bit5=ATAPI, bit6=WRITE, bit10=CLR_BUSY), prdtl@0x02, prdbc@0x04, **tbl_addr@0x08** | `typedef struct AHCICmdHdr { uint16_t opts; uint16_t prdtl; uint32_t status; uint64_t tbl_addr; ... } QEMU_PACKED` |
| 4 | CFL=0x20 实为 **opts bit5 = AHCI_CMD_ATAPI** (0x20!) | `#define AHCI_CMD_ATAPI (1<<5)`; 正确 CFL=0x02 (128B 表) + 写命令补 WRITE(0x40) |
| 5 | **PRDT flags_size 0-based** (size-1) @offset12 | `(flags_size & MASK) + 1`; 标准教程写 dbc 于 offset 8 (QEMU 是 reserved!) |
| 6 | **DMA 缓冲必须 guest-physical** (M121 任务页表 虚拟≠物理) | 用户 va=0x400920 → walk 得 phys=0x400920 (恒等); 每任务页表下必须转换 |
| 7 | **SeaBIOS 探测命令占用 IDE 引擎** → handle_cmd busy 早退 (PxCI 残留) | `handle_cmd` 开头 `status & (BUSY\|DRQ)` → 早退; **命令重试 4 次** (清 PxCI 重发) |
| 8 | 参考盘被 T3 写脏 → demo 非幂等 (二次运行读 0xAB) | demo 结尾恢复 i=7 模式 |

**方法论**: QEMU 9.2 源码 (raw.githubusercontent) 直读 = 规格真相 (教程/OSDev 与 QEMU 实现有出入 —— **设备行为以模拟器源码为准, 铁律 17 的"源码取证"变体**)。

## 3. 踩坑

1. **Q35 vs i440fx**: AHCI 在 i440fx 上手工 `-device ich9-ahci` 挂载行为异常 (SeaBIOS legacy 通道混用); **q35 原生 ich9-ahci (slot31.func2) 为标准路径** —— q35 也是更接近真机的机器;
2. **pci_find 多功能 slot bug** (根本性): func0 不匹配 → break; 影响任何"设备在 func>0"场景;
3. **SEABIOS 会先行探测 AHCI 端口** (PxCLB/PxFB 被 BIOS 写入, 0xA1/0xEC/0xEF 自动序列) —— 驱动要幂等接管 (重写 CLB/FB + 停/启 PxCMD);
4. **BSS 预算**: AHCI 缓冲 3×4KB static → BSS 尾 0x2BAC30 (余 5KB, 下次加代码前先看).

## 4. 下一步

- **W20 p5 候选**: ATA→FJFS 卷落到 AHCI (kernel fjfs 选择 AHCI/ATA 背板); ATA PIO 多扇区/lba48 与 AHCI 一致性;
- 审计表 #5 闭环: 真机 SATA = AHCI (amd/Intel 控制器通用 PCI class 0x1,0x6);
- **回归**: 26/26 (m134 新增; m131-bbx 等不受 q35 影响 —— 每用例独立 QEMU)。
