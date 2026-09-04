/* m139_http.c — W21: 网络栈完整性 + 自托管闭环第一步
 *  UDP 客户端 "GET source" -> host UDP server 应答源码 (hello-clone.c)
 *  -> 存 /tmp/hello-clone.c (tmpfs) + /disk/hello-clone.c (FJFS 落盘)
 *
 * 说明 (W21 取证): TCP guest->host 数据段在 QEMU 9.2 slirp 被丢弃
 * (SYN 通/数据不通, 证据链 docs/80); UDP 校验=0 合法通道 (m124 已证)
 * —— clone 传输用 UDP, TCP 服务器完整性由 m125 (hostfwd 入站) 承担。
 *
 * QEMU: -netdev user,id=net0 -device virtio-net-pci,netdev=net0,mac=52:54:00:12:34:57
 * host: python UDP server @127.0.0.1:8077 应答源码
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
static void put16(unsigned char *p, u16 v) { p[0] = (unsigned char)(v >> 8); p[1] = (unsigned char)v; }
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
    for (i = 0; i < n; i += 2)
        sum += ((u32)p[i] << 8) | p[i + 1];
    while (sum >> 16) sum = (sum & 0xFFFF) + (sum >> 16);
    return (u16)(~sum & 0xFFFF);
}

static u64 mac[6];
static unsigned char txbuf[1514];
static unsigned char rxbuf[1514];

static void build_udp(u16 sport, u16 dport, const unsigned char *payload, int plen)
{
    unsigned char *eth = txbuf;
    unsigned char *ip = txbuf + 14;
    unsigned char *ud = txbuf + 34;
    int i;
    static const unsigned char gw[6] = {0x52, 0x54, 0x00, 0x12, 0x34, 0x56};
    for (i = 0; i < 6; i++) { eth[i] = gw[i]; eth[i + 6] = (unsigned char)mac[i]; }
    put16(eth + 12, 0x0800);
    ip[0] = 0x45;
    ip[1] = 0;
    put16(ip + 2, (u16)(20 + 8 + plen));
    put16(ip + 4, 1);
    put16(ip + 6, 0x4000);
    ip[8] = 64;
    ip[9] = 17;
    put16(ip + 10, 0);
    put32(ip + 12, 0x0A00020F);
    put32(ip + 16, 0x0A000202);
    put16(ip + 10, cksum(ip, 20));
    put16(ud, sport);
    put16(ud + 2, dport);
    put16(ud + 4, (u16)(8 + plen));
    put16(ud + 6, 0); /* UDP cksum 0 = 合法 (不校验) */
    for (i = 0; i < plen; i++)
        ud[8 + i] = payload[i];
}

static void send_arp_reply(const unsigned char *req)
{
    unsigned char *eth = txbuf;
    unsigned char *ar = txbuf + 14;
    int i;
    for (i = 0; i < 6; i++) { eth[i] = req[22 + i]; eth[i + 6] = (unsigned char)mac[i]; }
    put16(eth + 12, 0x0806);
    put16(ar, 1); put16(ar + 2, 0x0800);
    ar[4] = 6; ar[5] = 4;
    put16(ar + 6, 2);
    for (i = 0; i < 6; i++) ar[8 + i] = (unsigned char)mac[i];
    put32(ar + 14, 0x0A00020F);
    for (i = 0; i < 6; i++) ar[18 + i] = req[22 + i];
    for (i = 0; i < 4; i++) ar[24 + i] = req[28 + i];
    sy(0x8A05, (long)txbuf, 60, 0, 0, 0);
}

static unsigned char body[1024];
static int body_len = 0;

static void run(void)
{
    static const char h[] = "m139: udp source clone (W21)\n";
    wr(h, sizeof(h) - 1);
    int pass_all = 1;
    u64 info[9];
    int i;
    for (i = 0; i < 9; i++) info[i] = 0;
    sy(0x8A04, (long)info, 0, 0, 0, 0);

    wrstr("m139: T1 net ready\n");
    if (info[0] == 1) { wrstr("m139:   net ok\n"); }
    else { wrstr("m139:   net FAIL\n"); pass_all = 0; }
    for (i = 0; i < 6; i++) mac[i] = info[1 + i];

    /* T2: 发 UDP 请求 (GET-SOURCE) 到 10.0.2.2:8077 */
    static const char req[] = "GET-SOURCE hello-clone.c\r\n";
    build_udp(40001, 8077, (const unsigned char *)req, sizeof(req) - 1);
    {
        long s = sy(0x8A05, (long)txbuf, 14 + 20 + 8 + (int)(sizeof(req) - 1), 0, 0, 0);
        wrstr("m139: T2 udp req sent rc=");
        wrdec((u64)s);
        wrstr("\n");
        if (s != 0) { pass_all = 0; }
    }

    /* T3: 收 UDP 响应 (src 10.0.2.2:8077) */
    {
        u64 spins = 0;
        while (spins < 400000000) {
            long rc = sy(0x8A06, (long)rxbuf, 1514, 0, 0, 0);
            if (rc > 0) {
                if (rxbuf[12] == 0x08 && rxbuf[13] == 0x06 && rc >= 42) {
                    u32 tpa = ((u32)rxbuf[38] << 24) | ((u32)rxbuf[39] << 16) |
                              ((u32)rxbuf[40] << 8) | rxbuf[41];
                    if (rxbuf[20] == 0 && rxbuf[21] == 1 && (tpa == 0x0A00020F))
                        send_arp_reply(rxbuf);
                    continue;
                }
                if (rxbuf[12] == 0x08 && rxbuf[13] == 0x00 && rc >= 42 && rxbuf[14 + 9] == 17) {
                    /* UDP: 头在 ip+20 */
                    u16 spt = (u16)((rxbuf[14 + 20] << 8) | rxbuf[14 + 21]);
                    u16 dpt = (u16)((rxbuf[14 + 20 + 2] << 8) | rxbuf[14 + 20 + 3]);
                    if (spt == 8077 && dpt == 40001) {
                        u16 ulen = (u16)((rxbuf[14 + 20 + 4] << 8) | rxbuf[14 + 20 + 5]);
                        int plen = (int)ulen - 8;
                        int k;
                        if (plen > 0 && body_len + plen <= 1024) {
                            for (k = 0; k < plen; k++)
                                body[body_len + k] = rxbuf[14 + 20 + 8 + k];
                            body_len += plen;
                        }
                        wrstr("m139: T3 udp resp len=");
                        wrdec((u64)body_len);
                        wrstr("\n");
                        break;
                    }
                }
            }
            spins++;
        }
    }

    /* T4: body 含 "cloned" 标记 + 落盘 */
    {
        int found = 0;
        wrstr("m139: T4 body\n");
        if (body_len > 0) {
            int k;
            for (k = 0; k + 6 < body_len; k++) {
                if (body[k] == 'c' && body[k + 1] == 'l' && body[k + 2] == 'o' &&
                    body[k + 3] == 'n' && body[k + 4] == 'e' && body[k + 5] == 'd') { found = 1; break; }
            }
        }
        if (!found) { wrstr("m139:   marker FAIL\n"); pass_all = 0; }
        else { wrstr("m139:   marker ok\n"); }
        if (body_len > 0) {
            long fd = sy(2, (long)"/tmp/hello-clone.c", 2, 0, 0, 0);
            if (fd >= 3) {
                sy(1, fd, (long)body, (long)body_len, 0, 0);
                sy(3, fd, 0, 0, 0, 0);
                wrstr("m139: T5 tmpfs save ok\n");
            } else { wrstr("m139:   tmpfs save FAIL\n"); pass_all = 0; }
            fd = sy(2, (long)"/disk/hello-clone.c", 2, 0, 0, 0);
            if (fd >= 3) {
                sy(1, fd, (long)body, (long)body_len, 0, 0);
                sy(3, fd, 0, 0, 0, 0);
                wrstr("m139: T6 fjfs save ok\n");
            } else { wrstr("m139:   fjfs save FAIL\n"); pass_all = 0; }
        }
    }

    if (pass_all) {
        static const char m2[] = "m139: M139 RESULT: PASS\n";
        wr(m2, sizeof(m2) - 1);
    } else {
        static const char f[] = "m139: M139 RESULT: FAIL\n";
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
