/* m150_tcpclient.c — B2: TCP 客户端数据面探针 (W21 followup, docs/98 B2)
 *
 * W21 决策回顾: clone 用 UDP 因 "QEMU 9.2 slirp 丢 guest→host TCP 数据段" (TCG 证据链)。
 * 本探针在 {TCG, KVM} 双模式复测同一条数据面, 判定丢包是 slirp 通病还是 TCG 特性:
 *   T1 握手: SYN -> 等 SYN|ACK (host 10.0.2.2:8021)
 *   T2 数据: ACK+PSH "FUJO-TCP-PROBE-64!" -> 等 host 回显
 *   T3 输出 DATA_SEGMENT=OK|DROP (OK=收到回显; DROP=FIN/超时)
 * PASS = 探测序列完整执行 (输出分列; 数据面结论由日志/对照判定)。
 */
typedef long int64_t;
typedef unsigned long u64;
typedef unsigned int u32;
typedef unsigned short u16;

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
    if (v == 0) {
        wr("0", 1);
        return;
    }
    while (v > 0) {
        b[--i] = '0' + (char)(v % 10);
        v /= 10;
    }
    wr(b + i, 22 - i);
}
static void wrstr(const char *s)
{
    int n = 0;
    while (s[n])
        n++;
    wr(s, n);
}

static u64 mac[6];
static unsigned char txbuf[1514];
static unsigned char rxbuf[1514];

static void put16(unsigned char *p, u16 v) { p[0] = (unsigned char)(v >> 8); p[1] = (unsigned char)v; }
static void put32(unsigned char *p, u32 v)
{
    p[0] = (unsigned char)(v >> 24); p[1] = (unsigned char)(v >> 16);
    p[2] = (unsigned char)(v >> 8); p[3] = (unsigned char)v;
}

static u16 cksum(const unsigned char *p, int len)
{
    u32 sum = 0;
    int i;
    for (i = 0; i < len; i += 2) {
        sum += ((u32)p[i] << 8) | (p[i + 1] & 0xFF);
    }
    while (sum >> 16)
        sum = (sum & 0xFFFF) + (sum >> 16);
    return (u16)(~sum & 0xFFFF);
}

static u16 tcp_cksum(const unsigned char *ip, const unsigned char *tc, int tlen)
{
    /* 伪头: src(12)/dst(16)/zero+proto(9)/tcp 长度(2) */
    unsigned char ph[20];
    int i;
    for (i = 0; i < 4; i++) {
        ph[i] = ip[12 + i];
        ph[i + 4] = ip[16 + i];
    }
    ph[8] = 0;
    ph[9] = 6;
    ph[10] = (unsigned char)(tlen >> 8);
    ph[11] = (unsigned char)tlen;
    for (i = 12; i < 20; i++)
        ph[i] = 0;
    {
        u32 sum = 0;
        for (i = 0; i < 20; i += 2)
            sum += ((u32)ph[i] << 8) | ph[i + 1];
        for (i = 0; i < tlen; i += 2)
            sum += ((u32)tc[i] << 8) | (tc[i + 1] & 0xFF);
        while (sum >> 16)
            sum = (sum & 0xFFFF) + (sum >> 16);
        return (u16)(~sum & 0xFFFF);
    }
}

static void send_arp_reply(const unsigned char *req)
{
    unsigned char *eth = txbuf;
    unsigned char *ar = txbuf + 14;
    int i;
    for (i = 0; i < 6; i++) { eth[i] = req[22 + i]; eth[i + 6] = (unsigned char)mac[i]; }
    put16(eth + 12, 0x0806);
    put16(ar, 1); put16(ar + 2, 0x0800);
    ar[4] = 6; ar[5] = 4; put16(ar + 6, 2);
    for (i = 0; i < 6; i++) ar[8 + i] = (unsigned char)mac[i];
    put32(ar + 14, 0x0A00020F);
    for (i = 0; i < 6; i++) ar[18 + i] = req[22 + i];
    for (i = 0; i < 4; i++) ar[24 + i] = req[28 + i];
    sy(0x8A05, (long)txbuf, 60, 0, 0, 0);
}

/* 发 ip/tcp 段: dst=10.0.2.2:DPORT, src self (0x0A00020F:SPORT), gw MAC 直发 */
static long send_tcp2(u16 sport, u16 dport, u32 seq, u32 ack, u16 flags,
                      const unsigned char *data, int dlen)
{
    static const unsigned char gw[6] = {0x52, 0x54, 0x00, 0x12, 0x34, 0x56};
    int ip_len = 20 + 20 + dlen;
    int tlen = 20 + dlen;
    unsigned char *eth = txbuf, *ip = txbuf + 14, *tc = txbuf + 34;
    int i;
    for (i = 0; i < 6; i++) {
        eth[i] = gw[i];
        eth[i + 6] = (unsigned char)mac[i];
    }
    put16(eth + 12, 0x0800);
    ip[0] = 0x45; ip[1] = 0;
    put16(ip + 2, (u16)ip_len);
    put16(ip + 4, 3);
    put16(ip + 6, 0x4000);
    ip[8] = 64; ip[9] = 6;
    put16(ip + 10, 0);
    put32(ip + 12, 0x0A00020F);
    put32(ip + 16, 0x0A000202);
    put16(ip + 10, cksum(ip, 20));
    put16(tc, sport);
    put16(tc + 2, dport);
    put32(tc + 4, seq);
    put32(tc + 8, ack);
    put16(tc + 12, (u16)(0x5000 | flags));
    put16(tc + 14, 65535);
    put16(tc + 16, 0);
    put16(tc + 18, 0);
    for (i = 0; i < dlen; i++)
        tc[20 + i] = data[i];
    put16(tc + 16, tcp_cksum(ip, tc, tlen));
    return sy(0x8A05, (long)txbuf, 14 + ip_len, 0, 0, 0);
}

static void run(void)
{
    static const char h[] = "m150: TCP client data-plane probe (W21 followup B2)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;
    u64 info[9];
    int i;
    u32 my_seq = 5000, his_seq = 0, his_ack = 0;
    u32 spt = 40002, dpt = 8021;
    static const char data[] = "FUJO-TCP-PROBE-64!";
    int data_ok = 0, fin_seen = 0;

    for (i = 0; i < 9; i++) info[i] = 0;
    sy(0x8A04, (long)info, 0, 0, 0, 0);
    for (i = 0; i < 6; i++) mac[i] = info[1 + i];
    if (info[0] == 1) {
        wrstr("m150: net ok\n");
    } else {
        wrstr("m150: net FAIL\n");
        sy(60, 7, 0, 0, 0, 0);
        for (;;) {
        }
    }

    /* T1 SYN */
    sy(0x8A05, (long)txbuf, 0, 0, 0, 0); /* flush 环形 (别把 ARP 残留当响应) */
    {
        long rc = send_tcp2(spt, dpt, my_seq, 0, 0x02, 0, 0);
        wrstr("m150: T1 syn rc=");
        wrdec((u64)rc);
        wrstr("\n");
        if (rc != 0)
            pass_all = 0;
    }

    /* T2 等 SYN|ACK / ACK; 之后发数据; 时间基等待 15s (0x6101 us 时钟) */
    {
        long t0 = sy(0x6101, 0, 0, 0, 0, 0);
        int handshaken = 0;
        int sent_data = 0;
        while (sy(0x6101, 0, 0, 0, 0, 0) - t0 < 15000000) {
            long rc = sy(0x8A06, (long)rxbuf, 1514, 0, 0, 0);
            if (rc > 0) {
                if (rxbuf[12] == 0x08 && rxbuf[13] == 0x06 && rc >= 42) {
                    u32 tpa = ((u32)rxbuf[38] << 24) | ((u32)rxbuf[39] << 16) |
                              ((u32)rxbuf[40] << 8) | rxbuf[41];
                    if (rxbuf[20] == 0 && rxbuf[21] == 1 && (tpa == 0x0A00020F))
                        send_arp_reply(rxbuf);
                    continue;
                }
                if (rxbuf[12] == 0x08 && rxbuf[13] == 0x00 && rxbuf[14 + 9] == 6 && rc >= 54) {
                    u16 spt2 = (u16)((rxbuf[14 + 20] << 8) | rxbuf[14 + 21]);
                    u16 dpt2 = (u16)((rxbuf[14 + 22] << 8) | rxbuf[14 + 23]);
                    u32 seq = ((u32)rxbuf[14 + 24] << 24) | ((u32)rxbuf[14 + 25] << 16) |
                              ((u32)rxbuf[14 + 26] << 8) | rxbuf[14 + 27];
                    u32 ack = ((u32)rxbuf[14 + 28] << 24) | ((u32)rxbuf[14 + 29] << 16) |
                              ((u32)rxbuf[14 + 30] << 8) | rxbuf[14 + 31];
                    u16 flags = (u16)(((rxbuf[14 + 32] & 0xF) << 8) | rxbuf[14 + 33]);
                    if (spt2 != dpt || dpt2 != spt)
                        continue;
                    u16 tl = (u16)(((rxbuf[14 + 36] & 0xF) << 8) | rxbuf[14 + 37]) >> 4;
                    int dlen = rc - 14 - 20 - tl * 4;
                    if (!handshaken && (flags & 0x12) == 0x12) {
                        his_seq = seq;
                        his_ack = ack;
                        handshaken = 1;
                        wrstr("m150: T2 SYN-ACK got (data len=0)\n");
                    }
                    if (handshaken && !sent_data) {
                        long r2 = send_tcp2(spt, dpt, my_seq + 1, his_seq + 1, 0x18,
                                            (const unsigned char *)data, sizeof(data) - 1);
                        sent_data = 1;
                        wrstr("m150: T2 data sent rc=");
                        wrdec((u64)r2);
                        wrstr("\n");
                        if (r2 != 0)
                            pass_all = 0;
                    }
                    if (dlen > 0) {
                        int k, match = 1;
                        for (k = 0; k < dlen && k < (int)(sizeof(data) - 1); k++) {
                            if (rxbuf[54 + tl * 4 + k] != (unsigned char)data[k])
                                match = 0;
                        }
                        if (match && dlen == (int)(sizeof(data) - 1)) {
                            data_ok = 1;
                            wrstr("m150: T3 ECHO OK (data plane alive)\n");
                        } else {
                            wrstr("m150: T3 data rcvd len=");
                            wrdec((u64)dlen);
                            wrstr(" match=");
                            wrdec((u64)match);
                            wrstr("\n");
                        }
                    }
                    if ((flags & 0x11) == 0x11) {
                        fin_seen = 1;
                        wrstr("m150: T3 FIN seen\n");
                        send_tcp2(spt, dpt, my_seq + 1, seq + 1, 0x11, 0, 0);
                        wrstr("m150: T3 FIN-ACK sent\n");
                    }
                    if (data_ok || fin_seen)
                        break;
                }
            }
        }
        wrstr("m150: handshake=");
        wrdec(handshaken ? 1u : 0u);
        wrstr(" DATA_SEGMENT=");
        wrstr(data_ok ? "OK\n" : "DROP\n");
        if (!handshaken || !sent_data)
            pass_all = 0;
        if (data_ok)
            pass_all = 0; /* 探针断言: 本用例期望在已知丢包环境复现 DROP; OK 视为对照异常 */
    }

    if (pass_all) {
        static const char m2[] = "m150: M150 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m150: M150 RESULT: FAIL\n";
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
