// hello-fujo.rs — 用 rustc 裸机目标编译的 ELF64 样本（无 clang 时的真实工具链回归）
//
// 编译: rustc --target x86_64-unknown-none -C linker=rust-lld \
//        -C link-arg=-T sdk/fujo.ld -C panic=abort sdk/hello-fujo.rs -o sdk/build/hello-fujo.elf
// 之后可直接 fujopack。

#![no_std]
#![no_main]

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // 真实程序将在 M1 syscall gate 上线后执行 linux-x64 syscall
    loop {
        core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
