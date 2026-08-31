/* hermes.c — M10 FujoOS Hermes CLI (ring3, linux ABI): agent 前端 + 小模型通道
 *
 * 流程 (调用顺序即双脑设计: agent 先启动, 再调小模型):
 *   启动 -> 0x5104 模型信息 -> 命令槽(内核捕获的首命令, 默认 'run') ->
 *   0x5101 意图分类 (engine=qwen: 内核经 COM2 -> 宿主 qwen_model_server.py
 *   -> Ollama qwen2.5:0.5b) -> 0x5102 fujoctx -> 工具路由 -> REPL:
 *   hermes> 读键盘 (0x5103) -> 回车后重复分类管线; exit/quit 退出。
 *
 * 编译 (scripts/build-kernel.ps1 自动执行):
 *   clang --target=x86_64-unknown-linux-gnu -O2 -nostdlib -static -fno-pie \
 *         -fuse-ld=lld -fno-builtin -Wl,-e,_start -Wl,-T,sdk/user/user.ld \
 *         sdk/hermes/hermes.c -o sdk/hermes/hermes.elf
 */
typedef long long int64_t;
typedef unsigned long long uint64_t;

/* 规范 syscall 包装 (M10 修正 M9 的 register-asm 拼装式包装, 用标准约束:
 * rax=编号, rdi/rsi/rdx=参数1/2/3 —— M9 的 intent/len 返回漂移 DEV 项根因候选) */
static int64_t sys3(long nr, long a, long b, long c) {
    int64_t ret;
    asm volatile("syscall"
                 : "=a"(ret)
                 : "a"(nr), "D"(a), "S"(b), "d"(c)
                 : "rcx", "r11", "memory");
    return ret;
}

static void puts(const char *s) {
    long n = 0;
    while (s[n] != 0) n++;
    sys3(1, 1, (long)s, n);
}

static void putc(char c) {
    char s[2];
    s[0] = c;
    s[1] = 0;
    puts(s);
}

static void pnum(long v) {
    char d[24];
    long i = 24, x = v;
    if (v == 0) {
        putc('0');
        return;
    }
    while (x > 0 && i > 0) {
        d[--i] = '0' + (char)(x % 10);
        x /= 10;
    }
    sys3(1, 1, (long)&d[i], 24 - i);
}

static void phex(uint64_t v) {
    const char *hex = "0123456789abcdef";
    char d[18];
    d[0] = '0';
    d[1] = 'x';
    for (int i = 0; i < 16; i++) d[2 + i] = hex[(v >> (4 * (15 - i))) & 0xF];
    sys3(1, 1, (long)d, 18);
}

static const char *intent_name(long i) {
    switch (i) {
    case 1: return "RUN";
    case 2: return "QUERY";
    case 3: return "OPEN";
    case 4: return "EXIT";
    default: return "UNKNOWN";
    }
}

static long strnlen(const char *s, long cap) {
    long i = 0;
    while (i < cap && s[i] != 0) i++;
    return i;
}

/* ---- 命令管线: 分类 -> 上下文 -> 工具路由 ---- */
static long do_command(const char *cmd, long len, int is_reply) {
    puts(is_reply ? "hermes: cmd='\"" : "hermes: cmd='");
    sys3(1, 1, (long)cmd, len);
    puts(is_reply ? "\"'\n" : "'\n");

    /* 模型调用原语: 意图分类 (engine=qwen) */
    long intent = sys3(0x5101, (long)cmd, len, 0);
    puts("hermes: intent=");
    phex(intent);
    puts(" ");
    puts(intent_name(intent));
    putc('\n');

    /* fujoctx: OS 上下文注入 */
    char ctx[256];
    long n = sys3(0x5102, (long)ctx, 255, 0);
    if (n > 0 && n < 256) ctx[n] = 0;
    puts("hermes: ctx[");
    pnum(n);
    puts("] = ");
    if (n > 0) puts(ctx);
    else puts("(empty)");
    putc('\n');

    /* 工具路由 (v0 单一工具; 大模型升级路径在此) */
    switch (intent) {
    case 1: /* RUN */
        puts("hermes: tool=module(user_test) planned\n");
        break;
    case 2: /* QUERY */
        puts("hermes: info=fujonn-engine qwen(com2) 五意图分类; 更多状态待M11\n");
        break;
    case 3: /* OPEN */
        puts("hermes: open=window(module) — M11 桌面路线\n");
        break;
    case 4: /* EXIT */
        puts("hermes: bye\n");
        sys3(60, 0, 0, 0);
        for (;;) {}
    default:
        puts("hermes: intent unknown — retry\n");
        break;
    }
    return intent;
}

void _start(void) {
    puts("hermes: v0.1 up - agent frontend (M10)\n");

    /* 模型信息 (0x5104) */
    char info[72];
    long n = sys3(0x5104, (long)info, 71, 0);
    if (n > 0 && n < 72) info[n] = 0;
    puts("hermes: model=");
    if (n > 0) puts(info);
    else puts("(none)\n");
    putc('\n');

    /* 首命令: 内核捕获槽 (默认 'run') */
    const char *slot = (const char *)0x402000;
    long slen = strnlen(slot, 64);
    if (slen > 0) {
        do_command(slot, slen, 0);
    }

    /* REPL: hermes> 读键盘 (0x5103) */
    char line[96];
    long idle = 0;
    const long IDLE_LIMIT = 1500L * 1000L * 1000L; /* ~60s 无输入自动退出 (验证模式) */
    for (;;) {
        puts("hermes> ");
        long ln = 0;
        for (;;) {
            long c = sys3(0x5103, 0, 0, 0);
            if (c == '\n') {
                break;
            } else if (c == 8) { /* backspace */
                if (ln > 0) {
                    ln--;
                    putc('\b');
                }
            } else if (c == 0) {
                idle++;
                if (idle > IDLE_LIMIT) {
                    puts("hermes: idle timeout - v0 exit\n");
                    sys3(60, 0, 0, 0);
                    for (;;) {}
                }
                continue;
            } else if (ln < 90) {
                line[ln++] = (char)c;
                putc((char)c);
            }
        }
        line[ln] = 0;
        idle = 0;
        long intent = do_command(line, ln, 1);
        if (intent == 4) {
            for (;;) {} /* do_command 内已 exit */
        }
    }
}
