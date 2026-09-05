#!/usr/bin/env python3
"""gen_goldset.py — B20: m141 goldset 36 -> 100 (deterministic generator).

Layout: anom known 0..19 (rules-covered) · anom novel-pos 20..29 · anom
novel-neg 30..39 · io 40..69 (mod-6 period, GT=next) · cls known 70..89
(dict commands) · cls novel 90..99 (out-of-dict verbs).
GT rules mirror B3: rate>=90 & wr=dead|diag -> 1; low rate & wr=ok|1 -> 0;
novel words rate<90 -> rules 0 (blow-up measured); io GT=(last+1)%6.
Prints C array block (paste into m141_eval.c).
"""
WEIRD = ["memleak", "zombie", "swapthrash", "spike", "leakhog", "zombiecmd",
         "thrashloop", "spikeburst", "memoryhang", "leakstorm"]
CLS_K = [("run the game", 1), ("run program", 1), ("execute script", 1),
         ("exec the demo", 1), ("run update", 1), ("exec now", 1),
         ("exit now", 4), ("quit program", 4), ("exit the shell", 4),
         ("quit all", 4), ("open file", 3), ("open the doc", 3),
         ("open directory", 3), ("open settings", 3), ("hello there", 2),
         ("hello system", 2), ("list apps", 3), ("list files", 3),
         ("build kernel", 1), ("what is the time", 2)]
CLS_N = [("launch the editor", 1), ("start the server", 1),
         ("what is the time now", 2), ("where is the log", 2),
         ("register a device", 3), ("setup the printer", 3),
         ("delete a file", 1), ("remove the cache", 1),
         ("shutdown the system", 4), ("terminate all tasks", 4)]

txt, duty, gt = [], [], []

def add(t, d, g):
    txt.append(t)
    duty.append(d)
    gt.append(g)

for i in range(20):                      # anom known 0..19
    if i % 2 == 0:
        add(f"ev pid={i} rate={92 + (i % 7)} wr={'dead' if i % 4 == 0 else 'diag'}", 2, 1)
    else:
        add(f"ev pid={i} rate={3 + (i % 6)} wr={'ok' if i % 4 == 1 else '1'}", 2, 0)
for i in range(10):                      # anom novel-pos 20..29 (rate<90)
    add(f"ev pid={20 + i} rate={70 + i} wr={WEIRD[i]}", 2, 1)
for i in range(10):                      # anom novel-neg 30..39
    add(f"ev pid={30 + i} rate={40 + i * 2} wr=ok", 2, 0)
for i in range(30):                      # io 40..69 (mod-6, GT=(last+1)%6)
    s, ln = i % 6, 4 + (i % 3)
    seq = " ".join(str((s + k) % 6) for k in range(ln))
    add(seq, 4, (s + ln) % 6)
for t, g in CLS_K:                       # cls known 70..89
    add(t, 1, g)
for t, g in CLS_N:                       # cls novel 90..99
    add(t, 1, g)

assert len(txt) == 100
print(f"#define NSAMP {len(txt)}")
print("static const char *S_TXT[NSAMP] = {")
for i in range(0, len(txt), 5):
    row = ", ".join('"' + t.replace('"', '\\"') + '"' for t in txt[i:i + 5])
    print("    " + row + ("," if i + 5 < len(txt) else ""))
print("};")
print("static int S_DUTY[NSAMP] = {")
for i in range(0, len(duty), 16):
    print("    " + ", ".join(str(d) for d in duty[i:i + 16]) + ",")
print("};")
print("static u64 S_GT[NSAMP] = {")
for i in range(0, len(gt), 16):
    print("    " + ", ".join(str(g) for g in gt[i:i + 16]) + ",")
print("};")
print("/* subsets */")
print("#define ANOM_KNOWN 20")
print("#define ANOM_NP 20  /* novel-pos start */")
print("#define IO_START 40")
print("#define CLS_NOVEL_START 90")
