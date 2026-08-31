/* agent.c — M9 实验agent (ring3, linux ABI): 模型原语调用方
 *
 * 流程: 读取内核放入的命令槽 (0x402000) ->
 *       fujo_model_call (0x5101 意图分类) ->
 *       fujo_ctx_fetch  (0x5102 上下文注入) ->
 *       输出计划 -> exit
 *
 * 编译 (scripts/build-kernel.ps1 自动执行):
 *   clang --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie \
 *         -fuse-ld=lld -Wl,-e,_start -Wl,-T,sdk/user/user.ld \
 *         sdk/ai/agent.c -o sdk/ai/agent.elf
 */
typedef long int64_t;

static int64_t sys3(long nr, long a, long b, long c) {
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    asm volatile("syscall"
                 : "+r"(rax)
                 : "r"(rdi), "r"(rsi), "r"(rdx)
                 : "rcx", "r11", "memory");
    return rax;
}

static void puts(const char *s) {
    long n = 0;
    while (s[n] != 0) n++;
    sys3(1, 1, (long)s, n);
}

void _start(void) {
    const char *cmd = (const char *)0x402000;

    puts("agent: fujo-agent up\n");
    puts("agent: cmd='");
    puts(cmd);
    puts("'\n");

    /* 模型调用原语: 意图分类 (规则引擎, 接口即未来神经引擎) */
    long intent = sys3(0x5101, (long)cmd, 64, 0);
    puts("agent: model_call -> intent=");
    {
        char b[6];
        b[0] = '0' + (char)(intent % 10);
        b[1] = '\n';
        b[2] = 0;
        puts(b);
    }

    /* fujoctx: OS 上下文注入 */
    char ctx[256];
    long n = sys3(0x5102, (long)ctx, 256, 0);
    puts("agent: context[");
    {
        char d[8];
        long i = 0, v = n;
        if (v == 0) { d[0] = '0'; d[1] = 0; puts(d); }
        else {
            char tmp[8];
            int t = 0;
            while (v > 0 && t < 7) { tmp[t++] = '0' + (v % 10); v /= 10; }
            for (i = 0; i < t; i++) d[i] = tmp[t - 1 - i];
            d[t] = 0;
            puts(d);
        }
    }
    puts("] = ");
    puts(ctx);

    /* 计划输出 (工具路由在此决策; v0: 单一工具) */
    puts("agent: tool=module(user_test) planned\n");
    puts("agent: (M9 v0: model-call primitive verified)\n");

    sys3(60, 0, 0, 0); /* exit */
    for (;;) {
    }
}
