# 93 · W30 — 真机就绪包: autostart + GRUB ISO + 发现 #17 (GRUB 交付中断帧差异)

> 里程碑: W30 (真机就绪包) · 计划: docs/91
> 一句话: **内核 mbi cmdline autostart (`fujo.run=<demo>` 直启, 真机无 sendkey)
> 完成并回归固化; WSL2 grub-mkrescue 引导 ISO 构建链打通; ISO/GRUB 引导验收暴露
> #17 —— GRUB 交付环境下 m1 首 tick iretq #GP (err=0x18 USER_DS), 与 QEMU
> -kernel 装载器路径差异, 记录为 W31 首任务。**

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

**ISO 引导验收 (发现 #17)**: ISO 启动日志完整到 `m1: sti, waiting first PIT tick...`
后 EXCEPTION vec=13 rip=0x20f502 (iretq), err=0x18 (USER_DS) —— GRUB 交付路径下
**首 tick 中断帧的 CS 槽呈 0x18** (GDT[3]=udata); QEMU -kernel 装载器同内核无此问题。
→ 候选根因 (未定): GRUB 离开时的段/栈/帧状态 vs multiboot loader 时序差异;
诊断动作 = 中断帧寄存器级 dump (W31 首任务); 处置口径: **参考机 TCG -kernel 不受影响**,
真机/ISO 引导波必须先行 #17 修复。

## 3. 平台差异审计表新增 (docs/74)

| # | 假设 | QEMU -kernel 装载器 | GRUB2/ISO | 状态 |
|---|---|---|---|---|
| 17 | 引导器交付中断上下文 | multiboot loader 交付帧正常 (m1 首 tick OK) | **首 tick iretq #GP err=0x18 (CS=USER_DS)** | ⚠️ **W30 发现**: 诊断排队 W31 (真机引导前置) |

## 4. COM1 捕获模板 (真机 checklist 摘要)

```
# 真机 Checklist (W30 起, 物理机接入后)
1. 启动介质: fujo-boot.iso (grub-mkrescue) 写 U 盘/光驱 (dd / Rufus dd 模式);
2. 串口捕获: 主板 COM1 (DB9 或 USB 转串, 115200/8N1) → 宿主终端 tee 日志;
3. BIOS: 关闭 SecureBoot (multiboot v1 不需要但减少变数); 引导顺序 U 盘;
4. 启动参数: grub.cfg 已带 fujo.run=<demo> (无需键盘);
5. 验收: 串口日志出现 "boot: autostart" + demo PASS / 无 sendkey;
6. 已知风险: #17 (ISO/GRUB 中断帧) 未解, 真机首跑前先 QEMU-ISO 验证;
   走 ISO 引导的 demo 需 —— 先修 #17 (可能 GRUB 版本相关, 试 grub 版本/legacy bios 变体)。
```

## 5. 状态

- **W30**: autostart ✅ + ISO 构建 ✅ + #17 发现 (未解, 转 W31) + m148 回归用例;
- **W31** (docs/91): 修复/诊断 #17 (GRUB 中断帧) → ISO 引导 PASS → 第二硬件列
  (WSL2 KVM 试试; 物理机波待设备)。
