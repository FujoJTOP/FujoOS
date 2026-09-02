/* m125_tcp.c — W14b: 最小 TCP 服务器端回显 (SYN/SYN-ACK/ACK + 数据回显 + FIN; docs/65)
 *
 * QEMU: -netdev user,id=net0,hostfwd=tcp:127.0.0.1:18080-:8080
 *       -device virtio-net-pci,netdev=net0,mac=52:54:00:12:34:57,disable-modern=on
 * host 侧: python 连接 127.0.0.1:18080 发 32B payload -> 收回显 -> 关闭
 * 断言:
 *   T1 net_info: ready=1
 *   T2 收到 SYN (src@8080? 不: host client 连 guest:8080)
 *   T3 回显: recv data 与 host payload 一致 -> PASS
 *   T4 可选: FIN 处理
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

static u16 cksum(const unsigned char *p, int n)
{
    u32 sum = 0;
    int i;
    for (i = 0; i < n; i += 2) {
        sum += ((u32)p[i] << 8) | p[i + 1];
    }
    while (sum >> 16) sum = (sum & 0xFFFF) + (sum >> 16);
    return (u16)(~sum & 0xFFFF);
}

/* TCP 校验和: 伪头(12B) + TCP 段 */
static u16 tcp_cksum(const unsigned char *ip, const unsigned char *tcp, int tlen)
{
    unsigned char ph[40];
    int i;
    for (i = 0; i < 4; i++) {
        ph[i] = ip[12 + i];        /* src */
        ph[i + 4] = ip[16 + i];    /* dst */
    }
    ph[8] = 0;
    ph[9] = 6;
    put16(ph + 10, (u16)tlen);
    for (i = 0; i < tlen; i++)
        ph[12 + i] = tcp[i];
    if (tlen & 1)
        ph[12 + tlen] = 0;
    return cksum(ph, 12 + tlen + (tlen & 1));
}

static u64 mac[6];
static unsigned char txbuf[1514];
static unsigned char rxbuf[1514];

/* 构造并发送 ARP reply (应答 who-has 10.0.2.15; 请求者 sha@22 spa@28) */
static long send_arp_reply(const unsigned char *req)
{
    unsigned char *eth = txbuf;
    unsigned char *ar = txbuf + 14;
    int i;
    for (i = 0; i < 6; i++) {
        eth[i] = req[22 + i];
        eth[i + 6] = (unsigned char)mac[i];
    }
    put16(eth + 12, 0x0806);
    put16(ar, 1);
    put16(ar + 2, 0x0800);
    ar[4] = 6;
    ar[5] = 4;
    put16(ar + 6, 2);
    for (i = 0; i < 6; i++)
        ar[8 + i] = (unsigned char)mac[i];
    put32(ar + 14, 0x0A00020F);
    for (i = 0; i < 6; i++)
        ar[18 + i] = req[22 + i];
    for (i = 0; i < 4; i++)
        ar[24 + i] = req[28 + i];
    return sy(0x8A05, (long)txbuf, 60, 0, 0, 0);
}

/* 构造并发送 ip/tcp 报文 (从收到的帧取 src ip/port) */
static long send_tcp(const unsigned char *rx, u16 sport, u16 dport,
                     u32 seq, u32 ack, u16 flags,
                     const unsigned char *data, int dlen)
{
    int ip_len = 20 + 20 + dlen;
    int tlen = 20 + dlen;
    unsigned char *eth = txbuf;
    unsigned char *ip = txbuf + 14;
    unsigned char *tc = txbuf + 34;
    int i;
    static const unsigned char gw[6] = {0x52, 0x54, 0x00, 0x12, 0x34, 0x56};
    for (i = 0; i < 6; i++) {
        eth[i] = gw[i];
        eth[i + 6] = (unsigned char)mac[i];
    }
    put16(eth + 12, 0x0800);
    ip[0] = 0x45;
    ip[1] = 0;
    put16(ip + 2, (u16)ip_len);
    put16(ip + 4, 2);
    put16(ip + 6, 0x4000);
    ip[8] = 64;
    ip[9] = 6;
    put16(ip + 10, 0);
    put32(ip + 12, 0x0A00020F);
    for (i = 0; i < 4; i++)
        ip[16 + i] = rx[14 + 12 + i]; /* dst = 接收帧 src ip */
    put16(ip + 10, cksum(ip, 20));
    put16(tc, sport);
    put16(tc + 2, dport);
    put32(tc + 4, seq);
    put32(tc + 8, ack);
    put16(tc + 12, (u16)(0x5000 | flags)); /* offset 5, flags */
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
    static const char h[] = "m125: minimal TCP server echo (W14b)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;
    u64 info[9];
    u32 my_seq = 1000, his_seq = 0, his_ack = 0;
    u16 peer_port = 0;
    int state = 0; /* 0=listen 1=established 2=fin */
    int echoed = 0;
    int i;

    for (i = 0; i < 9; i++)
        info[i] = 0;
    sy(0x8A04, (long)info, 0, 0, 0, 0);
    for (i = 0; i < 6; i++)
        mac[i] = info[1 + i];
    wrstr("m125: T1 ready=");
    wrdec(info[0]);
    wrstr(" (expect 1)\n");
    if (!(info[0] == 1))
        pass_all = 0;

    /* 轮询收 TCP 帧 */
    {
        u64 spins = 0;
        while (spins < 200000000) {
            long rc = sy(0x8A06, (long)rxbuf, 1514, 0, 0, 0);
            if (rc > 0) {
                if (rxbuf[12] == 0x08 && rxbuf[13] == 0x06 && rc >= 42) {
                    /* ARP who-has 10.0.2.15 (op 大端@20, tpa@38) */
                    u32 tpa = ((u32)rxbuf[38] << 24) | ((u32)rxbuf[39] << 16) |
                              ((u32)rxbuf[40] << 8) | rxbuf[41];
                    if (rxbuf[20] == 0 && rxbuf[21] == 1 && tpa == 0x0A00020F)
                        send_arp_reply(rxbuf);
                    continue;
                }
                if (rxbuf[12] == 0x08 && rxbuf[13] == 0x00 && rc >= 54) {
                    if (rxbuf[14 + 9] == 6) {
                        u16 dport = (u16)((rxbuf[14 + 20 + 2] << 8) | rxbuf[14 + 20 + 3]);
                        u16 sport = (u16)((rxbuf[14 + 20] << 8) | rxbuf[14 + 20 + 1]);
                        u32 seq = ((u32)rxbuf[14 + 24] << 24) | ((u32)rxbuf[14 + 25] << 16) |
                                  ((u32)rxbuf[14 + 26] << 8) | rxbuf[14 + 27];
                        u16 flags = (u16)(((rxbuf[14 + 32] & 0x0F) << 8) | rxbuf[14 + 33]);
                        int tlen = rc - 34; /* tcp 段长 */
                        if (dport == 8080 && sport != 0) {
                            peer_port = sport;
                            if (state == 0 && (flags & 0x02)) { /* SYN */
                                his_seq = seq;
                                send_tcp(rxbuf, 8080, peer_port, 1000,
                                         his_seq + 1, 0x12, 0, 0); /* SYN|ACK */
                                my_seq = 1001;
                                state = 1;
                                wrstr("m125: T2 SYN got, SYN-ACK sent\n");
                            } else if (state == 1 && (flags & 0x08) && (flags & 0x10)) {
                                /* PSH|ACK: 回显数据 */
                                int dlen = tlen - 20;
                                u32 ack = seq + (u32)(dlen > 0 ? dlen : 0);
                                if (dlen > 0) {
                                    send_tcp(rxbuf, 8080, peer_port, my_seq, ack,
                                             0x18, rxbuf + 54, dlen); /* PSH|ACK 回显 */
                                    my_seq += (u32)dlen;
                                    echoed = 1;
                                    wrstr("m125: T3 echo sent dlen=");
                                    wrdec((u64)dlen);
                                    wrstr("\n");
                                }
                            } else if (state == 1 && (flags & 0x01)) {
                                /* FIN: 回 FIN|ACK, 完成 */
                                send_tcp(rxbuf, 8080, peer_port, my_seq,
                                         seq + 1, 0x11, 0, 0);
                                state = 2;
                                wrstr("m125: T4 FIN handled\n");
                                break;
                            }
                            /* 纯 ACK: 忽略 */
                        }
                    }
                }
            }
            spins++;
        }
    }

    if (pass_all && echoed) {
        static const char m2[] = "m125: M125 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m125: M125 RESULT: FAIL\n";
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
