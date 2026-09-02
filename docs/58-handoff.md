# 58 · 接手文档(新对话从这里开始)

> **给新对话的开场指引**:先读本文件,再按需读文档地图;工作目录切到 `D:\Dev\FujoOS`。
> 项目所有开发均在本仓库完成;本文档随项目状态更新(推进里程碑时同步刷新"当前状态"一节)。

---

## 1. 项目是什么

**FujoOS** —— 零第三方依赖的 x86_64 原生 OS(内核、驱动、窗口系统、游戏层、工具链、AI OS 层全栈自研),Rust `no_std` 内核,QEMU 参考机(TCG)为唯一实测平台。
当前主线:**AI OS** —— 把模型从"推理后端"变成"系统器官"(AI For Next)。

## 2. 仓库与环境(迁移后的最新事实)

| 项 | 值 |
|---|---|
| 工作目录 | `D:\Dev\FujoOS`(2026-09 从 `C:\Users\hooya\Documents\FujoOS` 迁移;旧目录保留为备份,勿再使用) |
| 分支 | `fujoos-ai-dev`(工作分支,所有里程碑提交在此) |
| 远端 | `https://github.com/FujoJTOP/FujoOS.git` |
| HEAD | `b6ac379`(57 路线图;M112–M115 均已完成并验证) |
| 主机 | Windows 11 / PowerShell / Rust 工具链 / QEMU 9.2 / LLVM clang / Python 3 / Ollama(qwen2.5:7b) |

**push 技巧**:代理 `127.0.0.1:20085` 时断时续 —— 先 `git push origin fujoos-ai-dev`(走代理),失败再 `git -c http.proxy= push origin fujoos-ai-dev`(直连);反之亦然,交替重试即可。

## 3. 当前状态(接手时的快照)

- **AI For Next(Wave 7)全部完成**:M112(shm-link+事件环+cap_exec+异常哨兵)、M113(计划执行器+IO 预测)、M114(NLC+环境扫描)、M115(五职责回归 + 基线对比)——四波均 7B PASS、fujoregress 9/9、已推送。
- **实测数据**(docs/44-Ext 表格):哨兵 100 分类 10 命中/0 误报;计划 隔离+恢复 2/0、验证 1;IO 预测 10/30(对照 LRU 0/30);NLC 策略应用 3 条、配置 1/0/24;环境 桌面/配置 2/2;M115 五职责 PASS。
- **延迟实测**:7B 冷推理 4.6s、warm 0.15–0.17s(CPU)。
- **W8 完成(M118 R3 时延协议 + M119 R1 公理化)**:shm 帧 v2 快照@t0+evw+crit、回包 TTL、wait_rsp 丢弃、0x8309 探针、0x830A 公理化自检(离线入 fujoregress);规格 docs/59。
- **W9 完成(M116 权限域)**:域 {cap 集合/地址空间/中断域} + 可撤销,爆炸半径定理可断言,系统域=全局槽 6 兼容;详情 docs/60。**load_end/pad 提至 0x2C0000/0x1C0000**(BSS 尾 0x2A4DA0)。
- **W10 完成(M120 蒸馏+自改进)**:R5 FJRU v1 确定性字节码规则引擎(0x830B,五职责规则优先 engine=3,模型调用率→0),R6 审计环捕获/导出(0x830C/D,IO 自监督命中标签);工具 tools/distill_rules.py(7B 归纳+保真度 100% 门),保真度曲线 fidelity.csv;fujoregress 12/12,7B 回归 PASS。
- **W11 完成(M121 独立地址空间)**:每任务页表链 + CR3 切换(进程隔离:同 VA 不同物),fork 堆页物理拷贝,mnumap 撤销补全;系统/隐式任务逐字节兼容;fujoregress 13/13,桌面冒烟正常;详情 docs/62。
- **W12 完成(M122 VFS 抽象+tmpfs+/dev/model0)**:模型即设备 open/write/read/close;tmpfs 命名内存文件;/dev/model0 与 0x5101 同核(R5 规则优先);fujoregress 14/14;详情 docs/63。
- **W13a 完成(M123 PCI 模型+virtio 骨架)**:PCI 配置空间读写原语+查找+命令使能;virtio-blk legacy 探测/vring/提交/轮询骨架(m123 检测级 PASS);fujoregress 15/15。
- **W13b 调查进行中**:QEMU monitor 取证(纯 legacy/BAR1=MSI-X/queue-size=256)+ STATUS 字节写回显但 DRIVER_OK 后自复位(队列无效);eliminated: 命令使能/BAR/偏移/宽度/queue-size=16;待选路径 = ①ring 布局按设备尺寸核对(KVM 源码 virtqueue 布局)②virtio 1.0 现代传输;详见 docs/64 §4。
- **阶段一完成,W11/W12 完成,W13a/b(PCI 模型 + 调查)进行中**;下一步 = W13c(virtio 数据路径收口)→ W14(TCP/IP)。

## 4. 构建与验证(铁律优先)

```powershell
# 1) 内核构建 —— 必须在 kernel/ 下,必须见 "Compiling fujo-kernel"
cd kernel; cargo build --release
#    (根目录 cargo build 只会编 tools,产物是旧的!)

# 2) 检查新符号已入镜像(llvm-nm)
llvm-nm kernel/target/x86_64-unknown-none/release/fujo-kernel | Select-String <SYM>

# 3) 展平 + 检查 BSS 尾 < 0x2A2000(pad 0x1A2000)
python tools/flatten_elf.py kernel/target/x86_64-unknown-none/release/fujo-kernel kernel/fujo-kernel.bin --pad 0x1A2000
llvm-readobj --sections kernel/fujo-kernel.bin | Select-String -Context 0,2 ".bss"   # 尾 < 0x2A2000

# 4) AI 职责验证(模型在线,服务器自管 monitor + 注入启动键)
python tools/verify_ai.py --demo m115_five --needle "M115 RESULT: PASS" --model qwen2.5:7b --timeout 300

# 5) 全回归
python tools/fujoregress.py      # 9/9
```

- **样例**:`sdk/linux/m11x_*.c`(entry `_start`,clang → `.elf`,`scripts/build-samples.ps1` 注册;demo 输出 `M1xx RESULT: PASS`)。
- **verify_ai.py 参数**:`--demo`(elf 名)/`--needle`(日志锚点)/`--model`/`--timeout`/`--boot-wait`/`--boot-keys`。
- **qwen_model_server.py**:连 QEMU COM2(tcp:4001)收/发模型帧 + 独占 monitor(telnet:4568)用 `pmemsave` 读帧、`sendkey` 注入启动键;环境变量 `FUJO_MODEL/FUJO_MON_PORT/FUJO_LINK_PORT/FUJO_BOOT_KEYS/FUJO_BOOT_WAIT`。**monitor 只接受首个连接**(后续连接静默),故服务器必须独享。

## 5. AI 层架构速览(改代码前先读)

| 文件 | 内容 |
|---|---|
| `kernel/src/ai.rs` | 五职责实现 + 规则兜底(`rules_anom/rules_plan/rules_nlc`)+ 意图路由/分类;`fujo_anom_run`(0x8304)…`fujo_env_scan`(0x8308) |
| `kernel/src/ctx.rs` | 结构态/摘要(M112 前已有)+ 5 类事件环(128×5 u64,~5KB BSS)+ `0x8002` 订阅/`0x8003` 事件/`0x8004` 注入/`0x8005` 结构;`EV_SUB` 掩码 bit=kind-1 |
| `kernel/src/capability.rs` | `cap_exec`(0x8105):动作位掩码 ACT_KILL/ISOLATE/LAUNCH/SET_CFG/RESUME/ACK,`ALL_ACTS=0x3F`,每次动作审计;`fujo_cfg_get`(0x8106);M91 能力表/审计原语 |
| `kernel/src/sched.rs` | `TASK_SUSPENDED=2`、`task_suspend/task_resume`、`exec_spawn`(单并发槽)、`terminate_current_and_next`、kill 返回 i64 |
| `kernel/src/mem.rs` | `demand_zero_init` 钉住 shm 页(PT_HEAP1[0]=0xA00000\|0x7 + invlpg) |
| `kernel/src/wmsg.rs` | `fujo_wm_create/remove` 推 EV_WINDOW(a=wid,b=1/0) |
| `kernel/src/syscall.rs` | 0x8002–05 / 0x8105–06 / 0x8304–08 分发;SYS_EV_COUNT 采样钩子(每 1000 次 syscall → `ctx::sys_note`) |
| `kernel/src/gamemode.rs` | 游戏模式查 cfg 3/4/5(工作时段禁玩 → -1) |

**shm-link 帧协议**(共享页 @ `0xA00000`,内核钉住):
- 头(16B):`magic=0x48534A46`("FJSH" LE)、`ver`、`seq u64`、`kind u32`、`len u32`
- `payload` @ `0x18`(≤ 1KB)、`ctx 文本` @ `0x800`(≤ 0x600)
- kind:1=classify 2=anomaly 3=PLAN 4=IO 5=NLC 6=ENV
- 模型回包以 **FRAME 头的 seq** 编号(不是触发帧的 seq);内核 `wait_rsp` 丢弃不匹配行,6s 超时走规则。

**系统主线不变式**(W8 将公理化):①模型永不能执行未授权动作(cap_exec 门)②每个动作有审计 ③模型缺席系统继续运行(rules 兜底)④失败计数并降级。**模型输出 = hint;规则兜底是最终裁判;无模型驻留(7B 在宿主机 Ollama,QEMU TCG 跑不动大模型);`0x5101/02/04` 契约不变。** 推理助手职责暂缓。

## 6. 十要铁律(W5 教训浓缩,详见技能 fujoos-pitfalls)

1. 构建只从 `kernel/` 进入,必须见 "Compiling fujo-kernel",新符号用 `llvm-nm` 确认。
2. 每加 static/数组查 BSS 尾 < 0x2A0000(现尾 0x29CE10,余 ~12.7KB);超界同步改 `flatten --pad`。
3. 系统内计时:syscall 内 PIT 被 SFMASK 屏蔽,用 `timer::sleep_us`(rdtsc)忙等;0x6101 校准跨 tick,校准前别采样 t0。
4. QEMU 一律后台 job;monitor sendkey 逐键注入(每键 120ms,`sendkey os` 是非法多字符键)。
5. 端口冲突(4568/4001/4002)→ `netstat -ano | findstr` 找到 PID taskkill;残留进程会互杀 QEMU(陷阱:verify 返回全零,查僵尸进程)。
6. pid 0 是合法任务;窗口 id = 槽+1 非单调;A2/动作解析以 " TAG" 为分隔符别按空格截断。
7. 第二用户程序用 `user-high.ld`(0x1000000)+ `map_high_user()`,并置位帧分配器位图 0..511,否则窗口镜像被按需零页覆盖。
8. 引导三分路由:无模块→桌面;含 `m108_desk`→代理;其余→shell(fujoregress 靠注入 `os run hermes`)。
9. demo 断言语义从内核实现反推(偏移/长度/字节序),先打印再断言;等串口 "timer: calibrated" 再采 t0。
10. `static` 上别写 `#[repr]`(E0517);对齐放容器类型;用户指针检查范围 0x400000..0xC00000 与 0x1000000..0x1080000。

## 7. 文档地图

- `docs/52` · AI For Next 蓝图(模型=器官,五职责,边界)——新对话必读
- `docs/53/54/55/56` · M112–M115 各波实现与踩坑
- `docs/57` · 长期路线图(短期 W8–W10 = 阶段一;长期 = 类 Linux 三阶段)——W8 执行依据
- `docs/44`(+44-Ext)· AI OS 验收与五职责基线表
- `docs/08` · M1–M100 总路线图(C 盘镜像;M101–115 在 docs/50/51/52+)
- `docs/51` · 项目状态总览
- 技能:`fujoos-pitfalls`(踩坑速查表,开发前过一遍)、`ponytail`(最小实现原则)

## 8. 下一步执行清单(W8 起步)

1. 通读 `docs/57` 阶段一 + 本文件第 5 节源码地图。
2. **R3 时延一致性协议**(改动小、先做):在 `kernel/src/ctx.rs`/`ai.rs` 给 shm 请求加 `快照@t0`,帧头/上下文带事件增量区间与过期标记;模型回包声明有效期;内核 `wait_rsp` 检查事件环增量,关键事件到达即丢弃建议走规则。配套 `tools/qwen_model_server.py` 回包格式扩展 + `sdk/linux/m118*` 或新 demo(带断言)。
3. **R1 公理化**:四条不变式各写一个可断言测试(模型离线时也跑),加入 `fujoregress` 或独立 `verify_invariants`。
4. 每波:改码 → kernel 构建(Compiling 确认)→ 新符号 nm 确认 → flatten(查 BSS 尾)→ verify_ai.py PASS → 特定文件 `git add` → commit → push(`fujoos-ai-dev`)。

> 工作约定:每次改动后自验再提交;只提交目标任务文件;完成一个里程碑即推送一次;文档随状态同步刷新本文件第 3 节。
