# FujoOS

> A zero-dependency, native x86_64 operating-system kernel — kernel, drivers,
> window system, game layer, development toolchain and the AI-OS layer are all
> self-built in this repository.
> Three-platform binary **loader subset**: minimal static samples of Linux ELF /
> macOS Mach-O / Windows PE32+ run in place, uniformly packaged as self-contained
> **`.run`** (FUJR v1).
> Note: "compatibility" means the loader/shim surface, not full userspace
> compatibility; **see "Known limitations" at the bottom** (in-kernel compiler is
> a C subset, ACPI high tables unmapped, reference machine is QEMU).

**Status: v1.0 → W20 real-hardware path → W29–W31 three execution modes →**
**W33/B20 trust-adaptive AI safety → W34–W36 FUFORALL** — 100 milestones
(docs/08-roadmap-100.md) plus W13–W36: virtio-blk/net + IPv4/TCP echo ·
VFS + busybox directory commands ·
ABI v1 freeze · in-kernel tcc self-hosting compile chain · SMP AP online · unified
audit ring · network closed loop (UDP source-clone → in-kernel compile → run) ·
**W20 de-QEMU specifics**: platform detection (Bochs VBE evidence chain → is_qemu)
+ LAPIC ICR dual-semantics runtime switch + GRUB2 real-machine boot path + AHCI
(SATA) real disk + FJFS real persistence read-back + >1GiB high-memory identity
mapping + PCI multifunction enumeration (m137 PASS) · **W22–W28 AI vertical six
waves** (m141–m147): three-engine quality contrast (auto/model/rules) · distillation
closed loop (AI_CALLS 38→≤1) · adversarial validation (reproducible blast radius) ·
IO ownership rejudgement (deterministic baseline) · all-five-duties
self-supervision · event-flow sentinel · **W29–W31 three execution modes**: TCG vs
WHPX vs KVM (fujoregress `--accel whpx` / `tools/kvm-run.sh`; AI waves identical;
mode limits documented: WHPX 36/37, KVM 37/38 — docs/92, docs/91) · **W32/W33
trust-adaptive domains** (quality ledger → dom_admit → domain width = f(quality);
A-class anti-abuse) · **B20 model sweep**: 15 local models × 100-sample goldset
with leave-one-model-out robustness (novel blind-spot coverage is a
family/instruction-following property, not size-monotone) · **B24 policy gate**
(cfg value-domain + τ invariant) · **W34 Windows files run perfectly**
(kernel32 30 + gdi32/user32 10 + msvcrt 31 shim APIs · `.run`/FUJR container ·
`#!fujoshell` `.shell` scripts; m152_win.exe zero-modification, docs/105) ·
**W35 scatter factory** (header-only POSIX libc subset + public-domain SHA-256
source, runs natively; docs/106) · **W36 BOX-BRIDGE design** (closed-source =
box interface, not LEGO; docs/108). Reference regression: **44/44** (TCG);
CI: 44 cases.

> **Paper.** *FUAI: A Measurement-Parameterized Safety Envelope for
> AI-Integrated Operating Systems* — **preprint (priority record)**:
> [Zenodo 10.5281/zenodo.22352904](https://zenodo.org/records/22352904) ·
> arXiv submission in progress (cs.OS; author Yuxuan Jiang). The manuscript
> is published with the venue record; its evidence appendix is regenerable
> from this repository.

## Feature overview

| Layer | Capabilities |
|---|---|
| Kernel | x86_64 long mode · IDT/GDT/TSS · PIT 100Hz · preemptive multitasking (affinity/balance stats) · userspace exception isolation · dual TSS + IRQ routing |
| Memory | virtual memory v0 · demand-zero pages · frame allocator · U-bit hardening · weighted mmap demand pages |
| ABI | ELF64 / Mach-O / PE32+ loaders · Linux x86-64 39 syscalls · darwin/win32 shim families · `.run` (FUJR) container |
| Storage | ATA PIO + FJFS 4MiB volume (format/persist, two-phase reboot PASS) · **AHCI/SATA real disk (ICH9 q35, W20)** · page cache/readahead · archive sandbox |
| Network | virtio-net legacy · IPv4/UDP round-trip (ARP reply) · minimal TCP server SYN/ACK/PSH echo · UDP clone closed loop (W21) |
| Platform | W20 real-machine path (platform detect/GRUB2/AHCI/PCI multifunction) · W29–W31 three-mode contrast (TCG / WHPX / KVM) |
| Graphics | VBE 1024x768x32 + LFB · 5x7 font · software raster rect/tri/line · blit/scale · shader bytecode VM |
| Input | PS/2 keyboard IRQ1 · mouse IRQ12 (8042 sequence/hit-test) · XInput · IME |
| Audio | AC97 · 4ch mixer + LPF/gain chain |
| AI OS | model channel (shared-memory frames + event ring) · five duties (sentinel/planner/io-predict/nlc/env) · engine select (model/rules/auto) · rule-book fallback + model-absence state · **capability domains + revoke** · **trust-adaptive admission** (quality ledger → dom_admit; τ_high 46 / τ_low 35, derived in the paper) · adversarial path (m144/m151: unauthorized kill denied + audited) |

## Quick start

```
# 1) Build
cd kernel; cargo build --release
python tools/flatten_elf.py kernel/target/x86_64-unknown-none/release/fujo-kernel kernel/fujo-kernel.bin --pad 0x1C0000

# 2) Boot (any demo as initrd; monitor injects "os run hermes")
qemu-system-x86_64 -m 256M -kernel kernel/fujo-kernel.bin -initrd sdk/linux/m30_linux.elf `
  -monitor telnet:127.0.0.1:4568,server,nowait -display none -no-reboot
# monitor: sendkey o s spc r u n spc h e r m e s ret

# 3) Regression / one-click
python tools/fujoregress.py                 # full 40/40 (TCG); --accel whpx for WHPX contrast
pwsh scripts/onebuild.ps1                   # hello/gui/game template build+run 3/3
```

## Verifiable evidence

- **Regression loop**: `python tools/fujoregress.py` full **40/40** (TCG reference;
  AI waves m141–m150 PASS per wave) · `--accel whpx` 36/37 and
  `tools/kvm-run.sh` / KVM matrix 37/38 (environment-limited cases only: m129 on
  WHPX, m126/m129 on nested KVM — docs/92, docs/91);
- **AI-wave online evidence**: `python tools/verify_ai.py --demo m141_eval ... --model qwen2.5:7b`
  (three-engine contrast; n=100 goldset) · `--evil` adversarial interception
  (m144/m151: unauthorized actions denied + audited, blast radius reproducible);
- **B20 model sweep**: `python tools/eval_models.py` (15 models × 100 samples,
  resumable) + `tools/loo_analysis.py` + `tools/boot_ci.py` (LOO robustness,
  bootstrap CIs) — data in the private evidence appendix, all regenerable;
- **Docs site**: [docs/index.html](docs/index.html) — single-file official site
  (GitHub Pages /docs);
- **Runtime architecture**: [assets/archify/fujoos-runtime.html](assets/archify/fujoos-runtime.html) +
  visual review sheets (1440x900 / 2048x1320, light/dark);
- **Real-machine evidence**: GRUB2 multiboot v1 boot path (docs/74 §4) · AHCI/SATA
  real-disk persistence read-back (docs/75/76/79) · LAPIC platform dual-semantics
  + CPUID/MSR probes (docs/74);
- **30-second repro**: the Quick start commands — headless QEMU → monitor inject
  `os run hermes`.

## Repository layout

```
kernel/        fujo-kernel (x86_64, no_std, 40+ modules: syscall dispatch/drivers/AI OS)
sdk/           samples (linux/win/mac/kit/hermes/user + templates)
tools/         flatten_elf / fujopack / fujorun / fujoregress / ci.py / eval_models /
               gen_goldset / boot_ci / tau_derivation / plot_models / qemu-kvm.ps1
scripts/       build-kernel.ps1 / cross-build.ps1 / onebuild.ps1
docs/          index.html (single-file official site) · index.md (index) · 08 (100 milestones)
               · 51 (status) · 57 (roadmap) · 58 (handoff) · 11..104 (milestones / waves)
```

## Documentation

- [Site] docs/index.html (single file) · [Site index] docs/index.md
- [v1.0 release] docs/49-release-notes.md · [Project status] docs/51-project-status.md
- [100-milestone roadmap] docs/08-roadmap-100.md · [Long-term roadmap] docs/57-long-roadmap.md
- [Handoff] docs/58-handoff.md (start new conversations here)
- [Platform contrast] docs/92-w29-platform-contrast.md · [AI wave summary] docs/89-w28-ai-vertical-summary.md
- [SDK tutorial] docs/29-sdk-close.md · [2D engine analysis] docs/10-2d-engine.md · [DXVK feasibility] docs/09-dxvk-feasibility.md

## Known limitations

- **AI inference is not end-side**: the kernel exposes weight pages/model
  card/audit/orchestration; actual inference rides the host link (COM2 → host
  model service). No on-device LLM.
- TCG interpretive execution (real machine/KVM expected 10-100x; M57 contrast
  surface exists — re-run the same demos there);
- FJFS multi-cluster write round-trips (M99: single-cluster readback + ATA write
  wait fixed; large files queued);
- ACPI tables >64MiB unmapped (M96 guard);
- WHPX contrast: INIT/SIPI injection refused (m129 N/A, docs/92 #16); legacy 8259
  path needs `kernel-irqchip=off` (docs/92 #15) — APIC-izing the interrupt
  architecture is a follow-up;
- In-kernel compiler is a C subset (single function); **real-machine path is open
  since W20** (GRUB2 boot / AHCI real disk / platform-detect primitives all
  demonstrated); remaining: real-machine video (INT 10h VBE) and USB driver
  surface.
