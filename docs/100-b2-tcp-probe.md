# 100 · B2 — TCP 客户端数据面双模式探针 (m150) — 结论修正: 方向反转

> 里程碑: B2 (docs/98) · 上游: W21 (docs/80) · W31 (KVM 列)
> 一句话: **m150 (干净 demo: 伪头缓冲正确) 证明 guest→host TCP 数据段实际**可到达
> host (tcpsrv rx 18B)**, 而**出站连接的 host→guest 响应方向**被 slirp 丢弃
> (guest 15s 无回显) —— W21 原始"guest→host 丢"结论是**修复前 demo 的 ph 越界
> 污染** (数据段构造非法被 slirp 拒绝, 误读为"转发限制"); UDP 决策不变但理由链修正。

## 1. 交付

| 部件 | 说明 |
|---|---|
| `sdk/linux/m150_tcpclient.c` | 最小 TCP 客户端探针: SYN→SYN-ACK→PSH 数据(伪头校验 20B 缓冲)→回显/FIN 侦测;
  输出 `handshake=N DATA_SEGMENT=OK\|DROP`; PASS=探测序列完整 (DROP 为已知环境预期) |
| `tools/fujoregress.py` | `tcp_server` host echo 线程 + **m150-tcpclient 用例** (回归 39→40) |
| `tools/kvm-m150.sh` | WSL2 KVM 对照 (host python echo + autostart) |
| 证据 | TCG 与 KVM 同: `DATA_SEGMENT=DROP` (guest 侧); **host 侧日志 `tcpsrv: rx 18B`** |

## 2. 方向反转 (关键修正)

| 方向 | 结果 | 说明 |
|---|---|---|
| guest→host 数据段 | **✅ 到达** (host `tcpsrv: rx 18B`) | 与 W21 原始断言**相反** |
| host→guest 响应 (出站连接) | **❌ 丢弃** (guest 15s 无回显) | 真正受限的方向 |
| host→guest (m125 hostfwd 入站) | ✅ | 与出站连接**不同机制** |

**判定 (修正)**: slirp 受限的是**出站 TCP 连接的响应方向**; W21 的"guest→host 数据段
被丢"是**修复前 demo 的 ph 越界污染** —— 伪头缓冲 (ph[48]) 溢出破坏用户栈 → 数据段
构造非法 → slirp 拒绝非法段; 修复越界后数据段**能到达 host**。W21 证据链 (docs/80 §2)
的若干条目须标注"m150 修正"。

## 3. 对 W21 决策的影响

- UDP 替代 **仍然正确** (升级回 TCP 也收不到响应体 → 闭环 demo 依然不可行),
  但理由链修正为: **slirp 出站连接响应方向受限**;
- docs/80 §2 更新: 原始取证 → 修正注记 (方向反转 + 污染确认);
- 升级路径 (B19): slirp 响应方向源码查证 / `-netdev socket/tap`。

## 4. 状态

- **B2 完成** (双模式 + 方向反转修正); fujoregress 39→**40 用例**;
- B19 保持 (路径: slirp 源码 / tap netdev); docs/98 B2 更新为修正结论。
