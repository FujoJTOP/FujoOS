# 35 — 权重 mmap 对象 (M86)

状态: ✅ 完成。验收: QEMU 串口 `M86 RESULT: PASS`, demo `sdk/linux/m86_wmap.c`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x7C01 | wmap_load(ptr, len) | 权重复制入内核库 (WLIB 0xF30000, 8KiB) |
| 0x7C02 | wmap_res(va, len) | 登记权重 VA 区 (需求段 0x800000..0xC00000) |
| 0x7C03 | wmap_stats(ptr) | u64×4: (pfa, pages, wlen, maps) |

## 2. 按需页

```
用户读权重 VA (未映射) → #PF (present=0, user) → fujo_pf_handler
  → wmap_fault(cr2):
      cr2 ∈ WMAP 区 → 帧分配 (frame_alloc_zero) → 从 WLIB 按页序
      拷贝 4KiB → 置 PTE (P|W|U) → invlpg → iretq 重试
  权重库与 .run 资源节 (M31 DATA) 的形态一致: 资源可换真模型文件。
```

## 3. 布局

- WLIB_A = 0xF30000 (0xF00000 backbuffer 末 + 0x2000 后;
  页缓存 0xF10000..0xF28000 之后, 恒等映射内);
- 权重 VA 与用户堆同 PT (PT_HEAP0/1), demand-zero 先于 wmap 无冲突。

## 4. 实测 (m86_wmap.elf)

```
m86  : weight-map va=0xb90000 len=4096 (demand pages)
m86  : wmap page va=0xb90000 (from weight lib)
m86: sum=00000007 pfa=00000001 pages=00000001 wlen=00001000
m86: M86 RESULT: PASS
```

- 读 4KB 权重 (vs blob 和) 一致; 1 次 #PF → 1 页装入。

## 5. 后续

- M87 模型卡 (权限/计费/审计) 以资源节元数据面扩展; M94 fupm
  安装模型 → WLIB 类资源区; M95 全生命周期验收。
