//! fujorun —— FujoOS 运行器（M0 版：校验 / 查看 / 导出）
//!
//! M1+ 的完整运行语义（装载、重定位、syscall gate、JIT）在此扩展：
//! 本版完成 .run 容器完整性校验，并为三种执行路径（native / ir / embed）
//! 预留 dispatch 骨架。
//!
//! 用法:
//!   fujorun <input.run>                # 校验 + 摘要
//!   fujorun <input.run> --dump         # 完整转储
//!   fujorun <input.run> --extract DIR  # 导出各 section 为文件
//!   fujorun <input.run> --run-embed    # (M3/M6) 装载 embed 二进制并执行

use std::env;
use std::fs;
use std::path::Path;
use std::process::exit;

use fujo_compat::run::{self, RunInfo, TAG_EMBED, TAG_ICON, TAG_IR, TAG_MANIFEST};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    let mut input: Option<String> = None;
    let mut action = Action::Validate;
    let mut extract_dir = String::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--dump" | "-d" => action = Action::Dump,
            "--validate" | "-v" => action = Action::Validate,
            "--extract" | "-x" => {
                action = Action::Extract;
                i += 1;
                extract_dir = args.get(i).cloned().unwrap_or_default();
            }
            "--run-embed" => action = Action::RunEmbed,
            a if a.starts_with('-') => { eprintln!("fujorun: unknown flag {a}"); exit(1); }
            _ => input = Some(args[i].clone()),
        }
        i += 1;
    }
    let input = match input {
        Some(s) => s,
        None => { eprintln!("fujorun: no input file"); usage(); }
    };

    let bytes = match fs::read(&input) {
        Ok(b) => b,
        Err(e) => { eprintln!("fujorun: read {input}: {e}"); exit(1); }
    };

    match run::read_run(&bytes) {
        Ok(info) => match action {
            Action::Validate => {
                println!("OK  {} : FUJR v{}.{}  {} sections  {} bytes",
                    input, info.version.0, info.version.1, info.section_count, info.total_size);
                println!("    all section hashes verified (FNV-1a)");
                if let Some(m) = run::section_data(&bytes, &info, TAG_MANIFEST) {
                    let text = String::from_utf8_lossy(m);
                    if let Some(l) = text.lines().find(|l| l.trim_start().starts_with("\"name\"")) {
                        let v = l.split(':').nth(1).unwrap_or("").trim().trim_matches('"');
                        println!("    name: {v}");
                    }
                }
            }
            Action::Dump => dump(&input, &bytes, &info),
            Action::Extract => extract(&input, &bytes, &info, &extract_dir),
            Action::RunEmbed => run_embed(&input, &bytes, &info),
        },
        Err(e) => {
            eprintln!("FAIL {input}: {e}");
            exit(2);
        }
    }
}

enum Action {
    Validate,
    Dump,
    Extract,
    RunEmbed,
}

fn usage() -> ! {
    eprintln!(
        "fujorun 0.1 — FujoOS runner\n\
         \n\
         usage: fujorun <input.run> [--validate | --dump | --extract DIR | --run-embed]"
    );
    exit(1);
}

fn dump(path: &str, bytes: &[u8], info: &RunInfo) {
    println!("FUJR container: {path}");
    println!("  version: {}.{}   sections: {}   size: {}", info.version.0, info.version.1, info.section_count, info.total_size);
    for (i, s) in info.sections.iter().enumerate() {
        println!("  [{i:02}] {:<10} flags={:#x} off={:#x} size={:#x} hash={:08x}", run::tag_name(s.tag), s.flags, s.offset, s.size, s.hash);
    }
    if let Some(m) = run::section_data(bytes, info, TAG_MANIFEST) {
        println!("  -- manifest --");
        for line in String::from_utf8_lossy(m).lines() {
            println!("  {line}");
        }
    }
    if let Some(ic) = run::section_data(bytes, info, TAG_ICON) {
        println!("  -- icon: {} bytes --", ic.len());
    }
}

fn extract(path: &str, bytes: &[u8], info: &RunInfo, dir: &str) {
    let d = Path::new(dir);
    fs::create_dir_all(d).unwrap_or_else(|e| {
        eprintln!("fujorun: create dir {dir}: {e}");
        exit(1);
    });
    for (i, s) in info.sections.iter().enumerate() {
        let data = &bytes[s.offset as usize..s.offset as usize + s.size as usize];
        let fname = format!("{:02}_{}.bin", i, run::tag_name(s.tag).to_ascii_lowercase());
        let p = d.join(&fname);
        fs::write(&p, data).unwrap_or_else(|e| {
            eprintln!("fujorun: write {}: {e}", p.display());
            exit(1);
        });
        println!("extracted {} ({} bytes)", p.display(), data.len());
    }
}

/// EMBED 路径的占位执行器：
/// M1 之后 → 装载到受信地址空间 + fujo-compat 识别 + syscall gate 挂接。
fn run_embed(path: &str, bytes: &[u8], info: &RunInfo) {
    let embed = match run::section_data(bytes, info, TAG_EMBED) {
        Some(e) => e,
        None => {
            eprintln!("fujorun: {path}: no EMBED section");
            exit(3);
        }
    };
    println!("fujorun: {path}");
    println!("  embed: {} bytes", embed.len());
    match fujo_compat::inspect(embed) {
        Ok(bi) => {
            println!(
                "  detect: {} / {} / {}-bit, entry {:#x}{}, endian {}",
                bi.format.as_str(),
                bi.arch.as_str(),
                bi.bits,
                bi.entry,
                if bi.pie { " (pie)" } else { "" },
                bi.endian
            );
            println!("  exec path: native (embed) — loader dispatch arrives in M1/M2");
            println!("  note: execution requires kernel syscall gate (linux ABI) or shim layer (win32/darwin)");
        }
        Err(e) => println!("  detect: {e}"),
    }
    if run::section_data(bytes, info, TAG_IR).is_some() {
        println!("  IR section present: cross-arch JIT path (M7)");
    }
}
