# 47 — live 镜像 + 安装器 (M98)

状态: ✅ 完成。验收: 两阶段 QEMU `M98 RESULT: PASS`。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x8701 | inst_install() | boot 模块 → /system/fujo-kernel.bin + bootcount |
| 0x8702 | inst_status(ptr) | (installed, kernel_size, volume_ok, boot_count) |

## 2. 模型

```
live: QEMU -kernel fujo-kernel.bin -drive file=root.img (FJFS 卷)
安装: boot 模块 (initrd ELF) 拷贝 → FJFS /system/fujo-kernel.bin
      bootcount: 读盘上值+1 写回 (跨重启递增)
```

## 3. 两阶段实测

```
qemu -m 256M -kernel kernel/fujo-kernel.bin \
     -initrd sdk/linux/m98_install.elf -drive file=.m98disk.img ...
阶段1 (新镜像): inst=1 ksz=6424 vol=1 boot=1  → PASS
阶段2 (同盘重boot): inst=1 ksz=6424 vol=1 boot=2 → PASS (盘上持久)
```

- boot=2 证明 bootcount 文件跨重启在盘上 (安装持久)。

## 4. 后续

- M99 签名/更新: 安装器的更新目标 (盘上 kernel 替换 = 升级路径);
- rootfs 化: /system/ 与 /boot/ 分离 + 引导器从盘加载内核
  (v0 保持 initrd 引导, 安装面已具备)。
