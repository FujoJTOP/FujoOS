//! fujopack —— FujoOS 打包器
//!
//! 把 PE / ELF / Mach-O 二进制"转译"为 `.run`（FUJR 容器）：
//!   1. 识别格式与架构（fujo-compat）
//!   2. 生成清单（manifest）：来源、目标、API 需求、依赖
//!   3. 写入容器：MANIFEST + EMBED（原始二进制）(+ 可选 ICON)
//!
//! 用法:
//!   fujopack <input> [-o out.run] [--name NAME] [--format auto|pe|elf|macho]
//!                    [--arch auto|x86_64|aarch64|...] [--raw] [--dump]
//!   fujopack --dump <input.run>

use std::env;
use std::fs;
use std::process::exit;
use std::time::{SystemTime, UNIX_EPOCH};

use fujo_compat::run::{self, RunMeta, RunPart, TAG_DATA, TAG_EMBED, TAG_ICON, TAG_MANIFEST};
use fujo_compat::{Arch, BinaryInfo, Format};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    if args.iter().any(|a| a == "--dump" || a == "-d") {
        let input = args.iter().find(|a| !a.starts_with('-')).expect("need input");
        match dump(input) {
            Ok(()) => {}
            Err(e) => { eprintln!("fujopack: {e}"); exit(1); }
        }
        return;
    }

    let mut input: Option<String> = None;
    let mut output = String::from("out.run");
    let mut name = String::from("app");
    let mut format_hint = None::<Format>;
    let mut arch_hint = None::<Arch>;
    let mut raw = false;
    let mut icon: Option<String> = None;
    // M17: 资源节 + 权限声明
    let mut resources: Vec<(String, String)> = Vec::new(); // (name, path)
    let mut perms: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => { i += 1; output = args.get(i).cloned().unwrap_or_default(); }
            "--name" => { i += 1; name = args.get(i).cloned().unwrap_or_default(); }
            "--format" => { i += 1; format_hint = args.get(i).and_then(|s| Some(Format::from_str(s))); }
            "--arch" => { i += 1; arch_hint = args.get(i).and_then(|s| Some(Arch::from_str(s))); }
            "--raw" => raw = true,
            "--icon" => { i += 1; icon = args.get(i).cloned(); }
            "--res" => { i += 1; if let Some(spec) = args.get(i) {
                if let Some((n, p)) = spec.split_once(':') {
                    resources.push((n.to_string(), p.to_string()));
                } else {
                    eprintln!("fujopack: --res expects NAME:PATH"); exit(1);
                }
            } }
            "--perm" => { i += 1; if let Some(p) = args.get(i) { perms.push(p.clone()); } }
            "-h" | "--help" => { usage(); }
            a if a.starts_with('-') => { eprintln!("fujopack: unknown flag {a}"); exit(1); }
            _ => input = Some(args[i].clone()),
        }
        i += 1;
    }
    let input = match input {
        Some(s) => s,
        None => { eprintln!("fujopack: no input file"); usage(); }
    };

    let bytes = match fs::read(&input) {
        Ok(b) => b,
        Err(e) => { eprintln!("fujopack: read {input}: {e}"); exit(1); }
    };

    // --- 识别 ---
    let mut info: Option<BinaryInfo> = None;
    if !raw {
        info = match fujo_compat::inspect(&bytes) {
            Ok(i) => Some(i),
            Err(e) => {
                eprintln!("fujopack: {input}: {e}");
                eprintln!("hint: use --raw to wrap bytes without parsing");
                exit(1);
            }
        };
        if let Some(h) = format_hint {
            if info.as_ref().unwrap().format != h {
                eprintln!(
                    "fujopack: format hint {} disagrees with sniffed {}",
                    h.as_str(),
                    info.as_ref().unwrap().format.as_str()
                );
                exit(1);
            }
        }
        if let Some(h) = arch_hint {
            if info.as_ref().unwrap().arch != h {
                eprintln!(
                    "fujopack: arch hint {} disagrees with detected {}",
                    h.as_str(),
                    info.as_ref().unwrap().arch.as_str()
                );
                exit(1);
            }
        }
    }
    let detected = info.clone();
    let (format, arch, bits, entry, pie) = match &detected {
        Some(i) => (i.format.code(), arch_code(i.arch), i.bits, i.entry, i.pie),
        None => (0, 0, 0, 0, false),
    };

    // --- 清单 ---
    // --- 组装 section 顺序: [manifest(占位)] [icon?] [embed] [res...] ---
    // --- 再按最终 section 编号生成 manifest 并回填 -------------------------
    let mut res_blobs: Vec<(String, Vec<u8>)> = Vec::new();
    for (rname, rpath) in &resources {
        match fs::read(rpath) {
            Ok(rdata) => res_blobs.push((rname.clone(), rdata)),
            Err(_) => { eprintln!("fujopack: resource {rpath} not readable"); exit(1); }
        }
    }
    let mut parts: Vec<RunPart> = Vec::new();
    parts.push(RunPart { tag: TAG_MANIFEST, flags: 0, data: Vec::new() }); // 占位, 稍后回填
    if let Some(icon_path) = &icon {
        if let Ok(icon_data) = fs::read(icon_path) {
            parts.push(RunPart { tag: TAG_ICON, flags: 0, data: icon_data });
        }
    }
    parts.push(RunPart { tag: TAG_EMBED, flags: 0, data: bytes });
    let mut res_sections: Vec<(String, usize)> = Vec::new();
    for (rn, rb) in &res_blobs {
        parts.push(RunPart { tag: TAG_DATA, flags: 0, data: rb.clone() });
        res_sections.push((rn.clone(), parts.len() - 1));
    }
    let manifest = build_manifest(&name, &detected, format, arch, bits, entry, pie, &res_sections, &perms);
    parts[0].data = manifest.into_bytes();

    let meta = RunMeta {
        uid: make_uid(),
        target_arch: arch_code(if let Some(a) = arch_hint { a } else { fallback_arch(&detected) }),
        base_arch: arch_code(if let Some(a) = arch_hint { a } else { fallback_arch(&detected) }),
        source_format: format,
        flags: 0,
        manifest_index: 0,
    };

    let container = run::write_run(&parts, &meta);
    if let Err(e) = fs::write(&output, &container) {
        eprintln!("fujopack: write {output}: {e}");
        exit(1);
    }

    println!("packed  {} -> {}", input, output);
    match &detected {
        Some(i) => println!(
            "  source: {} / {} / {} bit, entry {:#x}{}, endian {}",
            i.format.as_str(),
            i.arch.as_str(),
            i.bits,
            i.entry,
            if i.pie { " (pie)" } else { "" },
            i.endian
        ),
        None => println!("  source: raw (--raw)"),
    }
    println!(
        "  container: FUJR v{}.{}, {} bytes, {} sections",
        run::VERSION_MAJOR,
        run::VERSION_MINOR,
        container.len(),
        parts.len()
    );
    println!("  manifest: {}", output);
}

fn usage() -> ! {
    eprintln!(
        "fujopack 0.1 — FujoOS packer (PE/ELF/Mach-O -> .run)\n\
         \n\
         usage: fujopack <input> [-o out.run] [--name NAME] [--format auto|pe|elf|macho]\n\
         \x20                 [--arch auto|x86_64|aarch64|i386|arm] [--raw] [--icon FILE]\n\
         \x20       fujopack --dump <input.run>"
    );
    exit(1);
}

fn arch_code(a: Arch) -> u16 {
    match a {
        Arch::X86_64 => 1,
        Arch::AArch64 => 2,
        Arch::X86 => 3,
        Arch::Arm => 4,
        Arch::Unknown => 0,
    }
}

fn fallback_arch(info: &Option<BinaryInfo>) -> Arch {
    match info {
        Some(i) => i.arch,
        None => Arch::Unknown,
    }
}

fn make_uid() -> [u8; 16] {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut x = nanos ^ (std::process::id() as u64) << 32;
    let mut uid = [0u8; 16];
    for b in uid.iter_mut() {
        // xorshift64
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = x as u8;
    }
    uid
}

fn json_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn build_manifest(
    name: &str,
    detected: &Option<BinaryInfo>,
    format: u32,
    arch: u16,
    bits: u8,
    entry: u64,
    pie: bool,
    res_sections: &[(String, usize)],
    perms: &[String],
) -> String {
    let src_fmt = match format {
        1 => "pe",
        2 => "elf",
        3 => "macho",
        _ => "raw",
    };
    let src_arch = match arch {
        1 => "x86_64",
        2 => "aarch64",
        3 => "i386",
        4 => "arm",
        _ => "unknown",
    };
    // API 子系统预测：后续由兼容层按导入符号精确填充
    let subsys = match format {
        1 => r#"["win32"]"#,
        2 => r#"["linux"]"#,
        3 => r#"["darwin"]"#,
        _ => r#"["raw"]"#,
    };
    // M17: 资源节引用 (section 号指向 TAG_DATA 段) + 权限声明
    let mut res_json = String::from("[");
    for (i, (rn, sec)) in res_sections.iter().enumerate() {
        if i > 0 {
            res_json.push_str(", ");
        }
        res_json.push_str(&format!(r#"{{"name":"{}","sec":{}}}"#, json_escape(rn), sec));
    }
    res_json.push(']');
    let perms_json = perms.iter().map(|p| format!("\"{}\"", json_escape(p))).collect::<Vec<_>>().join(", ");
    format!(
        "{{\n  \"manifest\": \"fujo.os.run/v1\",\n  \"name\": \"{}\",\n  \"source\": {{\n    \"format\": \"{}\",\n    \"arch\": \"{}\",\n    \"bits\": {},\n    \"entry\": \"{:#x}\",\n    \"pie\": {}\n  }},\n  \"target\": {{\n    \"arch\": \"x86_64\",\n    \"abi\": \"fujo\"\n  }},\n  \"exec\": \"embed\",\n  \"resources\": {},\n  \"perms\": [{}],\n  \"api\": {{\n    \"subsystems\": {},\n    \"shim_modules\": []\n  }},\n  \"libs\": [],\n  \"env\": {{}},\n  \"signature\": {{\n    \"alg\": \"none\",\n    \"note\": \"M8: ed25519\"\n  }}\n}}\n",
        json_escape(name),
        src_fmt,
        src_arch,
        bits,
        entry,
        pie,
        res_json,
        perms_json,
        subsys,
    )
}

fn dump(path: &str) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| format!("read {path}: {e}"))?;
    let info = run::read_run(&bytes)?;
    println!("FUJR container: {}", path);
    println!(
        "  version      : {}.{}",
        info.version.0, info.version.1
    );
    println!("  sections     : {}", info.section_count);
    println!("  total size   : {} bytes", info.total_size);
    println!("  uid          : {}", hex16(&info.uid));
    println!(
        "  target/base  : arch {} / {}  source-format {}",
        info.target_arch, info.base_arch, info.source_format
    );
    println!(
        "  flags        : {:#x}  manifest section #{}",
        info.flags, info.manifest_index
    );
    println!();
    println!("  {:<10} {:>8} {:>12} {:>12}  hash", "tag", "flags", "offset", "size");
    for (i, s) in info.sections.iter().enumerate() {
        println!(
            "  [{:02}] {:<10} {:>8} {:>12} {:>12}  {:08x}",
            i, run::tag_name(s.tag), s.flags, s.offset, s.size, s.hash
        );
    }
    if let Some(m) = run::section_data(&bytes, &info, TAG_MANIFEST) {
        println!();
        println!("  -- manifest --");
        for line in String::from_utf8_lossy(m).lines() {
            println!("  {line}");
        }
    }
    Ok(())
}

fn hex16(b: &[u8; 16]) -> String {
    let mut s = String::with_capacity(32);
    for x in b {
        s.push_str(&format!("{x:02x}"));
    }
    s
}
