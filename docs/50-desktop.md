# 50 — 桌面可操作面 (M101/M102: Win1.0 级交互闭环)

状态: ✅ M101/M102 完成 (QEMU `M101 RESULT: PASS` / `M102 RESULT: PASS`)。

## 目标

把已具备的原语 (M46 桌面 / M44 图标 / M39 字体 / M37-38 窗口 / M54 定时器)
串成"可操作"闭环: **点开始 → 菜单 → 开窗 → 画内容 → 关窗**, 及窗口
**拖动 / 焦点 / 层级**。全部用户态编排, 无内核改动。

## M101 桌面整合 (m101_desktop.c)

```
desk_init + taskbar("FujoOS 1.0")  → 图标×2 (0x5904)
消息循环 8 帧:
 [0][4] 点开始按钮 (0x5B03) → 菜单 (0x5B04)
 [1][5] 点 Programs → 0x5520 注册类 → 0x5521 开窗 (校验 >0)
 [2][6] 标题栏 0x6202 + 0x5601 字体 + 正文
 [3][7] 0x5524 关窗 → 桌面还原
实测: openings=2 menu=2 closes=2 → PASS
```

## M102 拖动/焦点/层级 (m102_windrag.c)

| 原语 | 确认语义 |
|------|----------|
| 0x5520 wm_class(name) | 注册类 → id (create 必需, 未注册 -22) |
| 0x5521 wm_create(class,x,y,w,h) | → winId (1 起, 槽+1) |
| 0x5525 wm_move(win,dx,dy) | **增量** (非绝对) |
| 0x5526 wm_rect(win,ptr) | 写 (x,y,w,h) 四元组 (非 x1y1x2y2) |
| 0x5523 wm_top(win) | 移至表顶 (z 序), 0/-2 |
| 0x5524 wm_remove(win) | 析构 |

```
双窗 A/B (id 1/2): rect (30,40,320,220)/(200,120,280,180)
拖动 B 6×(50,45) → (500,390); top/remove 链; 删除后 rect=-2
实测: winA=1 winB=2 moved=1 topB=0 rmB=0 → PASS
```

## 意义

- **Win1.0 级最小可操作面达成**: 桌面 → 鼠标点选 → 窗口开/关/拖动;
- 余下 (M103-106): 菜单/对话框模板、文本框光标编辑、图形程序文件
  打开/保存 (FJFS 后端)、鼠标注入回归 —— 原语已齐, 均为编排层。

## M103-M106 进展

| 里程碑 | 交付 | 实测 |
|--------|------|------|
| M103 | fujokit kt_menu (栏 22px/项 64px) + kt_dialog (OK/Cancel) | menu_sel=1 ok_hits=2 cancel=1 PASS |
| M104 | caret 感知 insert/backspace (删 caret-1) | "Hi"→插 X→退格→'s' = "sHi" PASS |
| M105 | 对话框 + VFS 磁盘文件保存/打开/读回 | wn=25 rn=25 一致 PASS |
| M106 | 全链操作回归 (①..⑤) | 1..5=TTTTT PASS |

M101-106 合计: 桌面整合 shell → 窗口拖动/层级 → 菜单/对话框 →
文本框编辑 → 文件对话框(持久) → 集成回归, 全部 QEMU 真原语验证。
窗口 id 语义记录: 槽+1 (remove 后复用, 非单调计数)。

## M107/M108 桌面会话 (boot → 图形桌面 → 双用户态任务)

| 里程碑 | 交付 | 实测 |
|--------|------|------|
| M107 | 内核态桌面主循环 (无模块 boot 直进桌面; 合成/真鼠标双击链) | `desktop shell up → window opened → alive` PASS (ttl 待用户态轮转) |
| M108 | **用户态桌面代理** (m108_desk.elf @0x400000, initrd 自启动无注入) + **高地址窗口程序** (hermes-high / tty-high @0x1000000, user-high.ld) | Hermes launch ok → Shell 替换 ok → 窗口程序写 TTY 行 (rows=14) → **M108 RESULT: PASS** |

M108 关键机制:
- **代理 = 任务 0**: `enter_user_test` 前 `sched::spawn_proxy` 登记 (kstack 0x380000 /
  用户栈 0x5FFFF8; 首次 PIT 用户态中断以真实现场覆盖 saved_rsp)。
- **窗口程序 = 任务 1+**: 0x5B10 `desk_launch` → 0x1000000 高区装载
  (mem::map_high_user: PD[8]→PT_HIGH, 2MiB U=1, 同步保留 512 帧防
  demand-zero 复用) → 独立 kstack 0x340000 / 用户栈 0x63FFF8。
- **PIT 双任务轮转**: 两任务同在用户态, cs==0x23 中断切换 (M107 内核态
  hlt 主循环切换不了用户任务的限制由此解除)。
- **TTY 行门控**: 仅窗口程序任务自身 (TTY_PID-1) 的 write(1) 计入 TTY_LINES,
  代理日志不伪造 rows (0x5B11 读回)。
- 代理内存模型: 与 M22 fork 同款"隐式任务 → 首次 PIT 覆盖帧"登记。

M108 时序修复 (实测): 校准前采样 t0 会因 cyc/us 突变产生 dt 跳变 → 先
0x6100 arm + 30×0x6104 轻等 (跨 syscall 边界让 PIT tick 落地) 再取 t0。
