/* m124_net.c — W14a: virtio-net 驱动 + 手工 ETH/IP/UDP echo 往返 (docs/65)
 *
 * QEMU: -netdev user,id=net0 -device virtio-net-pci,netdev=net0,queue-size=32
 *       -device 加 disable-modern=on (legacy)
 * 断言:
 *   T1 net_info: ready=1, mac 来自设备非全零
 *   T2 UDP TX: 构造 eth(14)+ip(20)+udp(8)+payload → net_tx 成功 (host 10.0.2.2:7777,
 *      hosts 侧 python tools/udp_echo.py 回显)
 *   T3 UDP RX: 收到回显帧, 校验 udp 端口/IP 与 payload 一致
 */
typedef long int64_t;
typedef unsigned long u64;
typedef unsigned short u16;
typedef unsigned int u32;

static int64_t sy(int64_t nr, int64_t a, int64_t b, int64_t c, int64_t d, int64_t e)
{
    register long rax asm("rax") = nr;
    register long rdi asm("rdi") = a;
    register long rsi asm("rsi") = b;
    register long rdx asm("rdx") = c;
    register long r10 asm("r10") = d;
    register long r8 asm("r8") = e;
    asm volatile("syscall" : "+r"(rax) : "r"(rdi), "r"(rsi), "r"(rdx),
                 "r"(r10), "r"(r8) : "rcx", "r11", "memory");
    return rax;
}

static void wr(const char *s, long len) { sy(1, 1, (long)s, len, 0, 0); }
static void wrdec(u64 v)
{
    char b[22];
    int i = 22;
    if (v == 0) { wr("0", 1); return; }
    while (v > 0) { b[--i] = '0' + (char)(v % 10); v /= 10; }
    wr(b + i, 22 - i);
}
static void wrstr(const char *s)
{
    int n = 0;
    while (s[n]) n++;
    wr(s, n);
}
static void wrhex(u64 v)
{
    static const char H[] = "0123456789abcdef";
    char b[20];
    int i;
    b[0] = '0'; b[1] = 'x';
    for (i = 0; i < 16; i++)
        b[2 + i] = H[(v >> (4 * (15 - i))) & 0xF];
    b[18] = 0;
    wr(b, 18);
}

static void put16(unsigned char *p, u16 v)
{
    p[0] = (unsigned char)(v >> 8);
    p[1] = (unsigned char)v;
}
static void put32(unsigned char *p, u32 v)
{
    p[0] = (unsigned char)(v >> 24);
    p[1] = (unsigned char)(v >> 16);
    p[2] = (unsigned char)(v >> 8);
    p[3] = (unsigned char)v;
}

/* 16-bit one's complement checksum (big-endian 头字段) */
static u16 cksum(const unsigned char *p, int n)
{
    u32 sum = 0;
    int i;
    for (i = 0; i < n; i += 2) {
        sum += ((u32)p[i] << 8) | p[i + 1];
        if (sum > 0xFFFFF) sum = (sum & 0xFFFF) + (sum >> 16);
    }
    while (sum >> 16) sum = (sum & 0xFFFF) + (sum >> 16);
    return (u16)(~sum & 0xFFFF);
}

static u64 mac[6];
static unsigned char txbuf[1514];
static unsigned char rxbuf[1514];

static void build_udp(u16 sport, u16 dport, const unsigned char *payload, int plen)
{
    int n = 14 + 20 + 8 + plen;
    unsigned char *eth = txbuf;
    unsigned char *ip = txbuf + 14;
    unsigned char *ud = txbuf + 14 + 20;
    int i;
    static const unsigned char gw[6] = {0x52, 0x54, 0x00, 0x12, 0x34, 0x56};
    for (i = 0; i < 6; i++) {
        eth[i] = gw[i];          /* dst = QEMU slirp 网关 */
        eth[i + 6] = (unsigned char)mac[i]; /* src = 本机 */
    }
    put16(eth + 12, 0x0800);     /* ethertype IPv4 */
    ip[0] = 0x45;
    ip[1] = 0;
    put16(ip + 2, (u16)(20 + 8 + plen));
    put16(ip + 4, 1);
    put16(ip + 6, 0x4000);       /* DF */
    ip[8] = 64;
    ip[9] = 17;                  /* UDP */
    put16(ip + 10, 0);
    put32(ip + 12, 0x0A00020F);  /* 10.0.2.15 */
    put32(ip + 16, 0x0A000202);  /* 10.0.2.2  */
    put16(ip + 10, cksum(ip, 20)); /* IP checksum */
    put16(ud, sport);
    put16(ud + 2, dport);
    put16(ud + 4, (u16)(8 + plen));
    put16(ud + 6, 0);            /* UDP checksum 0 = 不校验 (合法) */
    for (i = 0; i < plen; i++)
        ud[8 + i] = payload[i];
    (void)n;
}

/* 构造 ARP reply: 应答 "who has 10.0.2.15" (请求者 sha/spa 在 req 帧内) */
static void build_arp_reply(const unsigned char *req)
{
    unsigned char *eth = txbuf;
    unsigned char *ar = txbuf + 14;
    int i;
    for (i = 0; i < 6; i++) {
        eth[i] = req[22 + i];            /* dst = 请求者 sha */
        eth[i + 6] = (unsigned char)mac[i];
    }
    put16(eth + 12, 0x0806);
    put16(ar + 0, 1);                    /* htype */
    put16(ar + 2, 0x0800);               /* ptype */
    ar[4] = 6;
    ar[5] = 4;
    put16(ar + 6, 2);                    /* op = reply */
    for (i = 0; i < 6; i++)
        ar[8 + i] = (unsigned char)mac[i]; /* sha */
    put32(ar + 14, 0x0A00020F);          /* spa */
    for (i = 0; i < 6; i++)
        ar[18 + i] = req[22 + i];        /* tha = 请求者 sha */
    for (i = 0; i < 4; i++)
        ar[24 + i] = req[28 + i];        /* tpa = 请求者 spa (网络序拷贝) */
}

static void run(void)
{
    static const char h[] = "m124: virtio-net UDP echo (W14a)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;
    u64 info[9];
    int i;

    /* T1 驱动探测 */
    for (i = 0; i < 9; i++)
        info[i] = 0;
    sy(0x8A04, (long)info, 0, 0, 0, 0);
    for (i = 0; i < 6; i++)
        mac[i] = info[1 + i];
    wrstr("m124: T1 ready=");
    wrdec(info[0]);
    wrstr(" mac=");
    for (i = 0; i < 6; i++) {
        char b[4];
        static const char H[] = "0123456789abcdef";
        b[0] = H[(mac[i] >> 4) & 0xF];
        b[1] = H[mac[i] & 0xF];
        b[2] = 0;
        wr(b, 2);
        if (i < 5) wr(":", 1);
    }
    wrstr(" (expect ready=1 mac!=0)\n");
    if (!(info[0] == 1 && (mac[0] | mac[1] | mac[2] | mac[3] | mac[4] | mac[5]) != 0))
        pass_all = 0;

    /* T2 UDP TX */
    {
        static const unsigned char pl[32] = {
            'f', 'u', 'j', 'o', '-', 'n', 'e', 't', '-', 'w', '1', '4', 'a', '-', 'e', 'c',
            'h', 'o', '-', 'p', 'a', 'y', 'l', 'o', 'a', 'd', '-', '1', '2', '3', '4', '5'};
        build_udp(40000, 7777, pl, 32);
        long rc = sy(0x8A05, (long)txbuf, 74, 0, 0, 0);
        wrstr("m124: T2 tx rc=");
        wrdec((u64)rc);
        wrstr(" (0=ok)\n");
        if (rc != 0)
            pass_all = 0;
    }

    /* T3 UDP RX (轮询; 丢弃 ARP 前先应答, 保证 slirp 学到本机 MAC) */
    {
        long rc = 0;
        u64 spins = 0;
        while (spins < 80000000) {
            rc = sy(0x8A06, (long)rxbuf, 1514, 0, 0, 0);
            if (rc > 0) {
                if (rxbuf[12] == 0x08 && rxbuf[13] == 0x00 && rxbuf[14 + 9] == 17)
                    break; /* IPv4 + UDP */
                if (rxbuf[12] == 0x08 && rxbuf[13] == 0x06 && rc >= 42) {
                    /* ARP: 若询问 10.0.2.15 -> 应答 (op 字段大端) */
                    u32 tpa = ((u32)rxbuf[38] << 24) | ((u32)rxbuf[39] << 16) |
                              ((u32)rxbuf[40] << 8) | rxbuf[41];
                    if (rxbuf[20] == 0 && rxbuf[21] == 1 && tpa == 0x0A00020F) {
                        build_arp_reply(rxbuf);
                        sy(0x8A05, (long)txbuf, 60, 0, 0, 0);
                    }
                }
            }
            spins++;
        }
        wrstr("m124: T3 rx rc=");
        wrdec((u64)rc);
        wrstr(" (len>0=ok)\n");
        if (rc <= 0) {
            pass_all = 0;
        } else {
            /* 校验: eth dst=本机 mac, ethertype 0800, ip src 10.0.2.2,
               udp sport 7777 dport 40000, payload 一致 */
            int ok = 1;
            int j;
            for (j = 0; j < 6; j++)
                if ((unsigned char)rxbuf[j] != (unsigned char)mac[j])
                    ok = 0;
            if ((rxbuf[12] != 0x08) || (rxbuf[13] != 0x00))
                ok = 0;
            if (rxbuf[14 + 9] != 17)
                ok = 0;
            if (!((rxbuf[14 + 12] == 0x0A) && (rxbuf[14 + 13] == 0x00) &&
                  (rxbuf[14 + 14] == 0x02) && (rxbuf[14 + 15] == 0x02)))
                ok = 0;
            if (!((rxbuf[14 + 20] == 0x1E) && (rxbuf[14 + 21] == 0x61) &&
                  (rxbuf[14 + 22] == 0x9C) && (rxbuf[14 + 23] == 0x40)))
                ok = 0;
            if (ok && rc >= 70) {
                /* payload 比较 (第 74 字节起 = 14+20+8+32=74) */
                static const unsigned char pl[32] = {
                    'f', 'u', 'j', 'o', '-', 'n', 'e', 't', '-', 'w', '1', '4', 'a', '-', 'e', 'c',
                    'h', 'o', '-', 'p', 'a', 'y', 'l', 'o', 'a', 'd', '-', '1', '2', '3', '4', '5'};
                int k;
                for (k = 0; k < 32; k++)
                    if ((unsigned char)rxbuf[42 + k] != pl[k])
                        ok = 0;
            } else {
                ok = 0;
            }
            wrstr("m124: T3 verify=");
            wrdec((u64)ok);
            wrstr(" (1=ok)\n");
            if (!ok)
                pass_all = 0;
        }
    }

    if (pass_all) {
        static const char m2[] = "m124: M124 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m124: M124 RESULT: FAIL\n";
        wr(f, sizeof(f) - 1);
    }
    sy(60, 7, 0, 0, 0, 0);
    for (;;) {
    }
}

void _start(void)
{
    run();
    for (;;) {
    }
}
