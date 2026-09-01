# 28 — 工具链手册 (M79: fujopack/fujorun 全参数化)

状态: ✅ 完成。`--name/--type/-v` (fujopack), `--name/--smp/--timeout/
--bootsleep` (fujorun) 实测通过。

## 1. fujopack (.run 容器: FUJR v1)

```
python tools/fujopack.py pack -e EXEC [-m manifest.json]
        [-r name:file ...] [-o out.run]
        [--name app名] [--type app|game|tool] [-v|--verbose]
python tools/fujopack.py info  FILE.run      # 节表 (不校验)
python tools/fujopack.py check FILE.run      # 节表 + fnv1a 校验
```

格式 (kernel/src/fujr.rs):
- 64B 头 `[FUJR][ver u32][count u32]` + 32B×count 节表
  `[tag u32][pad u32][off u64][size u64][fnv1a u32][pad u32]` + payload。
- 节: 1=MANIFEST (json: name/type/resources/perms) · 4=EMBED · 5=DATA。

## 2. fujorun (BootMulti 多模块)

```
python tools/fujorun.py pack -i main.run|elf [--lib lib.run ...]
        -o multi.bin [--name main]
python tools/fujorun.py run -k kernel.bin -i main.run --lib lib.run
        [--mem 256M] [--smp N] [--keys "os spc run ..."]
        [--bootsleep 8.0] [--timeout S] [--log path]
```

格式: `FUJOMULT` + count u64 + 32B×count 条目 (off, len, name[16])
+ 模块数据 (模块 0 = 可执行体)。

## 3. 相关链工具

| 工具 | 用途 |
|------|------|
| tools/flatten_elf.py | 内核 ELF → fujo-kernel.bin (--pad 1A0000) |
| tools/fujoregress.py | 兼容矩阵 9 用例 |
| tools/ci.py | CI 25 用例 (兼容 + 里程碑日志断言) |
| tools/qemu-kvm.ps1 | KVM 加速启动对照 |

## 4. 端到端示范

```
python tools/flatten_elf.py kernel/target/x86_64-unknown-none/release/fujo-kernel \
    kernel/fujo-kernel.bin --pad 0x1A0000
python tools/fujopack.py pack -e sdk/linux/m77_win.elf -o app.run --name demo-app
python tools/fujorun.py run -k kernel/fujo-kernel.bin -i app.run \
    --keys "os spc run spc hermes" --timeout 20 --log .demo.log
```

## 5. 参数化实测

```
fujopack: sections=2 exec=16428b ... (--name demo-app --type game -v)
fujopack: [0] EMBED off=0x1000 ... fnv=0x12c2d13d ok   (info/check)
fujorun: wrote .m79_multi.initrd (22676 bytes, 2 modules)  (--name main)
```
