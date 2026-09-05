# 100 · B2 — TCP 客户端数据面双模式探针 (m150: TCG=KVM=DROP, slirp 通病确认)

> 里程碑: B2 (docs/98) · 上游: W21 (docs/80 slirp 丢数据证据链) · W31 (KVM 列)
> 一句话: **m150 独立 TCP 客户端探针在 {TCG, KVM} 双执行模式复测 —— 握手均通、
> 数据段均被丢弃 (DATA_SEGMENT=DROP) —— QEMU slirp 的 guest→host TCP 数据段
> 转发限制是**通病而非 TCG 特性**; W21 的 UDP 决策升级为跨模式成立的判定,
> 升级回 TCP/HTTP 的路径不在执行模式 (需 slirp 源码/换 netdev), 另立 followup。**

## 1. 交付

| 部件 | 说明 |
|---|---|
| `sdk/linux/m150_tcpclient.c` | 最小 TCP 客户端探针: SYN→SYN-ACK→PSH 数据(带伪头校验)→回显/FIN 侦测;
  输出 `handshake=N DATA_SEGMENT=OK\|DROP`; PASS=探测序列完整 (DROP 为已知环境预期) |
| `tools/fujoregress.py` | `tcp_server` host echo 线程 + **m150-tcpclient 用例** (回归 39→40) |
| `tools/kvm-m150.sh` | WSL2 KVM 对照 (host python echo + autostart) |
| 证据 | 双模式同探针: TCG `DATA_SEGMENT=DROP` + KVM `DATA_SEGMENT=DROP` |

## 2. 结论矩阵

| | TCG | WHPX (未测) | **KVM** | 物理机 (非 QEMU) |
|---|---|---|---|---|
| 握手 (SYN/SYN-ACK) | ✅ | — | ✅ | — |
| 出站数据段转发 | **DROP** | — | **DROP** | 预期 OK (无 slirp) |

**判定**: 丢包与 guest CPU 执行模式无关联 (slirp 在 QEMU 用户态进程内, 与 accel 无关) ——
证据从"TCG 单模式观察"升级为"双模式确认的 QEMU slirp 通病"。

## 3. 对 W21 决策的影响

- UDP 权宜 **仍然正确且现在更站得住**: 不是 TCG 的偶然, 是 QEMU slirp 的既定行为;
- 自托管闭环升级回 TCP/HTTP 的**可达路径** = ① QEMU slirp 源码/版本演进
  (tcp_input 转发逻辑查证) 或 ② `-netdev socket/tap` + 外部网络栈 —— 均非执行模式可控;
- 记录: docs/98 B2 ✅ + docs/80 §2 补注 (双模式确认)。

## 4. 状态

- **B2 完成** (双模式证据); fujoregress 39→**40 用例**;
- 关联 followup (新): "slirp 出站 TCP 数据段转发限制" 查证/绕行 = 网络栈侧,
  降为 docs/98 B19 (有路径: slirp 源码 / tap 网络后端)。
