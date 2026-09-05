#!/usr/bin/env python3
"""compat_audit.py — P4: 兼容 = 回归矩阵覆盖的依赖闭包 (度量自动化).

读 kernel/src/pe_loader.rs 的 SHIM_TABLE 与 sdk/win/*.c (demo 依赖),
报告:  (A) 已实现但无用例的 shim API (正例缺口);
      (B) 被 demo 使用但缺 SHIM_TABLE 项的导入 (编译期已保证不出现,
          此处扫描作为双检);
输出: 覆盖统计 + 缺口清单 (exit 非 0 当存在 A 类缺口)。
Usage: python tools/compat_audit.py
"""
import glob
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def shim_table():
    src = open(os.path.join(ROOT, "kernel", "src", "pe_loader.rs"), encoding="utf-8").read()
    pat = re.compile(r'\("([^"]+)", "([^"]+)", 0x([0-9A-Fa-f]+)\)')
    return {(m.group(1), m.group(2)) for m in pat.finditer(src)}


def demos():
    used = set()
    for fn in glob.glob(os.path.join(ROOT, "sdk", "win", "*.c")):
        txt = open(fn, encoding="utf-8", errors="replace").read()
        for m in re.finditer(r'__declspec\(dllimport\)[^;]*?\b(\w+)\s*\(', txt):
            used.add(m.group(1))
    return used


def main():
    table = shim_table()
    used = demos()
    by_name = {fn for _, fn in table}
    # A: shim 实现但无直接 demo 引用 (无用例)
    uncovered = sorted(fn for _, fn in table if fn not in used)
    # B: demo 引用但不在 shim 表 (双检; 正常应为空)
    missing = sorted(fn for fn in used if fn not in by_name)
    print(f"SHIM_TABLE entries: {len(table)} | demo-imported names: {len(used)}")
    print(f"\n[A] implemented but no demo case ({len(uncovered)}):")
    for fn in uncovered:
        print(f"    - {fn}")
    print(f"\n[B] used by demo but not in SHIM_TABLE ({len(missing)}):")
    for fn in missing:
        print(f"    - {fn}")
    print(f"\ncoverage: {(len(table) - len(uncovered)) / max(1, len(table)) * 100:.0f}% "
          f"({len(table) - len(uncovered)}/{len(table)})")
    return 1 if uncovered else 0


if __name__ == "__main__":
    raise SystemExit(main())
