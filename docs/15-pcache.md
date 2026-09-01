# 15 — 页缓存/预读 (M66, v0)

状态: ✅ 完成。验收: QEMU 串口 `M66 RESULT: PASS`, demo `sdk/linux/m66_pcache.c`。

## 1. 接口

| 编号  | 签名 | 说明 |
|-------|------|------|
| 0x6C01 | alloc(n) | 分配 n 页块逻辑空间 (块号=槽号 v0) |
| 0x6C02 | write(blk, ptr) | 用户页 → 缓存页 (置脏) |
| 0x6C03 | read(blk, ptr) | 缓存页 → 用户; miss → 从盘同步 (命中/hit 计数) |
| 0x6C04 | prefetch(start, n) | 顺序预读: 盘 → 缓存 (clean) |
| 0x6C05 | flush() | 脏页回写盘 (block 编码偏移) |
| 0x6C06 | evict() | 全部槽失效 (重载面) |
| 0x6C07 | info(ptr) | u32×4: (slots, dirty, hits, miss) |

## 2. 布局 — 选址教训 (重要)

- backbuffer = **0xC00000..0xF00000** (1024×768×32 = 3MiB)。
- 初版把缓存/磁盘数据区放 **0xD00000..0xDFxx00** —— 全部落在
  backbuffer 内! 实测: 几何徽章/窗口绘制把 **0x30305A** (窗口边框
  色) 的小端字节序列 (5A 30 30 00) 写到 0xDF2000/0xDF6000, 页缓存
  读回 0x5A。**检索出的 0x5A = 'Z' 实则颜色字面量字节**。
- 修正: CACHE_DATA = **0xF10000** (16 页 → 0xF20000 止),
  MEM_DISK = **0xF24000** (4 页 → 0xF28000 止), 均在 backbuffer
  之后、boot 0..64MiB 恒等映射内, 启动 `pcache::init()` 格式化清零。
- 教训: 常量物理区选址先核对 backbuffer/用户/栈区全部占用。

## 3. 实测 (m66_pcache.elf)

```
m66: base=00000000
pcache: flushed 2 dirty pages -> mem-disk        (页0=0xAB 页1=0xCD)
m66: flush_pages=00000002
m66: pf=00000002 r0=000000ab rpf0=000000ab rpf1=000000cd
pcache: miss blk=2 src=0x0000000000f26000 v=0x0000000000000000
m66: rmiss=00000000
m66: slots=00000001 dirty=00000000 hits=00000003 miss=00000001
m66: M66 RESULT: PASS
```

- write→dirty→flush 回盘; 缓存 hit 直读; evict→prefetch 从盘装回;
  未预读页 (blk 2) 走 miss→盘同步 (空页 0)。
- 统计: hits=3 (步骤4/5 直读 + 盘装后), miss=1 (blk2)。

## 4. 与真盘的衔接

- 模拟盘 (0xF24000) 3 页数据窗口 + 块号编码已按"块设备"语义抽象;
  真 ATA 路径接入时替换 `MEM_DISK + blk*PAGE` 读写为磁盘 I/O 即可
  (接口/脏位/预读窗口不变)。
