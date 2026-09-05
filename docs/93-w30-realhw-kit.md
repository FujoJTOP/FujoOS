# 93 · W30 — 真机就绪包: autostart + GRUB ISO + #17 解密 (SS=0x18 根因)

> 里程碑: W30 (真机就绪包) · 计划: docs/91
> 一句话: **内核 mbi cmdline autostart (`fujo.run=<demo>` 直启, 真机无 sendkey)
> 完成并回归固化; WSL2 grub-mkrescue 引导 ISO 构建链打通; #17 解密: GRUB 交付
> SS=0x18 (udata, 引导期从不重载) —— 长模式分段基址忽略使 SS 失效静默运行,
> 首个 iretq 复查 SS.RPL(3) vs CS.RPL(0) 矛盾 → #GP(err=0x18); 修复 = 入口
> 显式装载内核数据段 (0x10), ISO/GRUB 全链路 PASS。**

## 1. 交付

| 部件 | 说明 |
|---|---|
| `syscall.rs::boot_autostart` | mbi cmdline (flags bit2) 解析 `fujo.run=<name>`; 与 boot 模块名匹配 (contains) → 真 |
| `main.rs` 路由 | autostart 分支 (desk-proxy 后, shell 前): 命中 → `enter_user_test` 直启 |
| `main.rs rust64_entry` | **入口防御 cli** (加载器 IF 状态无关化; QEMU -kernel 与 GRUB 交付一致性) |
| `scripts/make-boot-iso.ps1` | WSL2 grub-mkrescue 构建 ISO (grub.cfg: `multiboot ... fujo.run=<Demo>` + `module`) |
| `tools/fujoregress.py` | `opts.append` + **m148-autostart 用例** (m142 via -append, 无 sendkey) |
| WSL 工具链 | Ubuntu 26.04 装入 grub-pc-bin/xorriso/mtools (grub-mkrescue ✓) |

## 2. 实测

**autostart (cmdline 路径, 等效真机 mbi cmdline)**:
```
boot: autostart (cmdline fujo.run) -> direct launch
fmod : '...m142_feedback.elf' @0x2c1000 9240 bytes
m142: ... M142 RESULT: PASS      ← 无 sendkey, 自动运行!
```
回归: m148-autostart PASS; autostart 缺省 (无 cmdline) = 旧路径 (37/37 保持)。

**ISO 构建**: `sdk/build/fujo-boot.iso` (7.37MB, GRUB2 + multiboot v1)。

**ISO 引导验收 → #17 解密 (已解决)**: ISO 启动完整到 m1 后 EXCEPTION vec=13
rip=iretq, err=0x18 —— **帧级观测 (tick_sched dump) 揭示**: 中断帧
[RIP][CS=0x08][RFLAGS][RSP=内核栈][**SS=0x18**] —— **根因 = SS 而非 CS**:
- GRUB 交付 SS=0x18 (其环境数据段); 引导桩仅 `ljmp` 装 CS, `gdt::init` 从不重载 SS;
- 长模式分段基址被忽略 → SS 失效**静默** (所有数据访问正常, m1 前一切"正常");
- 首个 iretq (段装载复查点) 检查 **SS.RPL(3) vs CS.RPL(0)** 矛盾 → #GP(err=SS=0x18);
- QEMU `-kernel` loader 交付 SS=0x10 (有效) → 不崩 → 加载器差异假象;
- **修复**: `rust64_entry` 入口显式 `mov ax,0x10; mov ss,ax` → **ISO/GRUB 全链路 PASS**:
  `cmdline → autostart → m142 RESULT: PASS` (无 EXCEPTION)。

## 3. 平台差异审计表新增 (docs/74)

| # | 假设 | QEMU -kernel 装载器 | GRUB2/ISO | 状态 |
|---|---|---|---|---|
| 17 | 引导期 SS 段 | SS=0x10 (有效内核段) | **SS=0x18 (udata) 且从不重载 → 首个 iretq #GP** | ✅ **W31 解密+修复**: 入口显式装载 SS=0x10 (compat.ts 经验: 64 位模式段装载放宽但装载点必查) |

## 4. COM1 捕获模板 (真机 checklist 摘要)

```
# 真机 Checklist (W30 起, 物理机接入后)
1. 启动介质: fujo-boot.iso (grub-mkrescue) 写 U 盘/光驱 (dd / Rufus dd 模式);
2. 串口捕获: 主板 COM1 (DB9 或 USB 转串, 115200/8N1) → 宿主终端 tee 日志;
3. BIOS: 关闭 SecureBoot (multiboot v1 不需要但减少变数); 引导顺序 U 盘;
4. 启动参数: grub.cfg 已带 fujo.run=<demo> (无需键盘);
5. 验收: 串口日志出现 "boot: autostart" + demo PASS / 无 sendkey;
6. #17 已解 (SS=0x10 显式装载) —— QEMU-ISO 与真机同路径, 首跑风险 = 0 (已知项)。
```

## 5. 状态

- **W30 完成**: autostart ✅ + ISO 构建 ✅ + **#17 解密修复 (SS=0x18 根因)** + m148 用例;
- W31 既有交付 (KVM 列) + #17 关闭 → **真机就绪包全部就绪**, 物理机波待设备。
