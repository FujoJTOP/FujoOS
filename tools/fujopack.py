#!/usr/bin/env python3
"""fujopack — FujoOS `.run` 容器命令行工具链 (M31)

FUJR v0.1 容器格式 (与 kernel/src/fujr.rs 一致):
  64B 头 [FUJR][ver u32][count u32][pad]
  + 32B×count 节表: [tag u32][pad u32][off u64][size u64][fnv1a u32][pad u32]
  + payload (4096 对齐)
  节 tag: 1=MANIFEST(json)  4=EMBED(可执行体)  5=DATA(资源)

用法:
  fujopack.py pack   -e EXEC [-m manifest.json] [-r name:file ...] -o out.run
  fujopack.py info   FILE.run
  fujopack.py check  FILE.run
"""
import argparse
import json
import sys


def fnv1a(data: bytes) -> int:
    h = 0x811C9DC5
    for b in data:
        h ^= b
        h = (h * 0x01000193) & 0xFFFFFFFF
    return h


def pack(exec_bytes: bytes, resources, manifest: str | None, out: str, name: str = "fujo-program", type_: str = "app", verbose: bool = False):
    sections = []
    sections.append((4, exec_bytes))  # EMBED
    man = None
    if manifest:
        man = manifest.encode()
    else:
        man = json.dumps({
            "name": name,
            "type": type_,
            "resources": [{"name": n} for (n, _) in resources],
            "perms": ["runres:read"],
        }).encode()
    sections.append((1, man))
    for (name_, data) in resources:
        sections.append((5, data))
    if verbose:
        print(f"fujopack: sections={len(sections)} exec={len(exec_bytes)}b resources={len(resources)}")

    count = len(sections)
    hdr_len = 64 + 32 * count
    off = hdr_len
    entries = []
    payload = bytearray()
    for (tag, data) in sections:
        aligned = (off + 4095) & ~4095
        payload.extend(b"\0" * (aligned - off))
        off = aligned
        payload.extend(data)
        entries.append((tag, aligned, len(data), fnv1a(data)))
        off += len(data)

    hdr = bytearray()
    hdr.extend(b"FUJR")
    hdr.extend((1).to_bytes(4, "little"))  # v0.1
    hdr.extend(count.to_bytes(4, "little"))
    hdr.extend(b"\0" * (64 - 12))
    for (tag, o, sz, h) in entries:
        sec = bytearray(32)
        sec[0:4] = tag.to_bytes(4, "little")
        sec[8:16] = o.to_bytes(8, "little")
        sec[16:24] = sz.to_bytes(8, "little")
        sec[24:28] = h.to_bytes(4, "little")
        hdr.extend(sec)
    hdr.extend(payload)
    with open(out, "wb") as f:
        f.write(hdr)
    print(f"fujopack: wrote {out} ({len(hdr)} bytes, {count} sections)")


def info(path: str, verify: bool):
    with open(path, "rb") as f:
        data = f.read()
    if data[:4] != b"FUJR":
        print("fujopack: not a FUJR container", file=sys.stderr)
        return 1
    ver = int.from_bytes(data[4:8], "little")
    count = int.from_bytes(data[8:12], "little")
    print(f"fujopack: FUJR v{ver} sections={count} total={len(data)}")
    names = {1: "MANIFEST", 4: "EMBED", 5: "DATA"}
    ok = True
    for i in range(count):
        base = 64 + i * 32
        tag = int.from_bytes(data[base:base + 4], "little")
        off = int.from_bytes(data[base + 8:base + 16], "little")
        size = int.from_bytes(data[base + 16:base + 24], "little")
        h = int.from_bytes(data[base + 24:base + 28], "little")
        real = fnv1a(data[off:off + size]) if verify else -1
        match = "ok" if (not verify or h == real) else f"HASH MISMATCH ({h:#x} vs {real:#x})"
        if match != "ok":
            ok = False
        print(f"  [{i}] {names.get(tag, tag):8s} off={off:#x} size={size:#x} fnv={h:#010x} {match}")
    return 0 if ok else 1


def main():
    ap = argparse.ArgumentParser(prog="fujopack")
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("pack")
    p.add_argument("-e", "--exec", required=True)
    p.add_argument("-m", "--manifest", default=None)
    p.add_argument("-r", "--resource", action="append", default=[])
    p.add_argument("-o", "--out", required=True)
    p.add_argument("--name", default="fujo-program", help="manifest 名")
    p.add_argument("--type", default="app", help="manifest 类型 (app/game/tool)")
    p.add_argument("-v", "--verbose", action="store_true", help="节表概要")
    p2 = sub.add_parser("info")
    p2.add_argument("file")
    p3 = sub.add_parser("check")
    p3.add_argument("file")
    a = ap.parse_args()
    if a.cmd == "pack":
        with open(a.exec, "rb") as f:
            ex = f.read()
        man = None
        if a.manifest:
            with open(a.manifest, "r", encoding="utf-8") as f:
                man = f.read()
        res = []
        for r in a.resource:
            if ":" not in r:
                print(f"fujopack: resource '{r}' must be name:file", file=sys.stderr)
                return 1
            name, path = r.split(":", 1)
            with open(path, "rb") as f:
                res.append((name[:15], f.read()))
        pack(ex, res, man, a.out, a.name, a.type, a.verbose)
        return 0
    if a.cmd == "info":
        return info(a.file, verify=False)
    if a.cmd == "check":
        return info(a.file, verify=True)
    return 1


if __name__ == "__main__":
    sys.exit(main())
