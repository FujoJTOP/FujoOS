// 目标校验守卫 (M9 记录): kernel 必须构建为 x86_64-unknown-none (ELF)。
// 该项目从仓库根目录用 `cargo build --manifest-path kernel\Cargo.toml` 调用时,
// cargo 基于 CWD 做配置发现, 读不到 kernel\.cargo\config.toml, 会错误地按宿主
// (MSVC/COFF) 构建 —— LLVM 的 COFF 汇编器会拒绝 ELF 指令 (.type/.size/.macro),
// 表现为 "expected absolute expression" / "unknown directive" 的"间歇性"报错,
// 并且最终会用 link.exe (LNK1561) 链接失败。
// 守卫: 构建脚本永远在宿主上运行, 在编译内核 crate 之前先检查 TARGET。
fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if target != "x86_64-unknown-none" {
        panic!(
            "fujo-kernel 必须构建为 x86_64-unknown-none (当前 TARGET={target})\n\
             请从 kernel/ 目录运行: cargo build --release\n\
             (仓库根目录配置发现不到 kernel\\.cargo\\config.toml, 会落到错误的目标工具链)"
        );
    }
}
