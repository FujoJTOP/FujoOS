#!/usr/bin/env python3
"""make_scatter_tool.py — 散件工厂: 拼装单编译单元工具 (tcc 无 GOT -> 单文件)。

输入 (sdk/scatter/): fujo_libc.h (散件) + sha256.h + sha256.c (原样公共域源码)
                     + fujo_main.c (测试驱动)
输出: sdk/scatter/sha256tool.c (拼装: libc + 工具源码 + main; 适配层最小化:
      仅 sha256.h 的 <stddef.h>/<stdint.h> 改为 fujo_libc.h)。
内核侧 shell.rs `sfactory` 命令 include_str! 本产物。
"""
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
D = os.path.join(ROOT, "sdk", "scatter")


def load(name):
    return open(os.path.join(D, name), encoding="utf-8").read()


def flatten_h(t):
    """单编译单元拼装: 去 include guards 与其 #endif (tcc 预处理器不稳)。"""
    t = re.sub(r"^#ifndef [A-Z0-9_]+_H\s*$", "", t, flags=re.M)
    t = re.sub(r"^#endif[^\n]*$", "", t, flags=re.M)
    t = re.sub(r"^#include <(stddef|stdint)\.h>\s*$", "", t, flags=re.M)
    t = re.sub(r"^#include \"(sha256|fujo_libc)\.h\"\s*$", "", t, flags=re.M)
    return t


def main():
    libc = flatten_h(load("fujo_libc.h"))
    h = flatten_h(load("sha256.h"))
    c = flatten_h(load("sha256.c"))
    m = flatten_h(load("fujo_main.c"))
    out = "/* sha256tool.c — 散件工厂拼装产物 (make_scatter_tool.py 生成) */\n" \
          "/* 组成: fujo_libc.h + sha256.h(适配) + sha256.c(原样) + fujo_main.c */\n\n" \
          + libc + "\n" + h + "\n" + c + "\n" + m
    dst = os.path.join(D, "sha256tool.c")
    open(dst, "w", encoding="utf-8").write(out)
    print(f"wrote {dst} ({len(out)} bytes)")


if __name__ == "__main__":
    main()
