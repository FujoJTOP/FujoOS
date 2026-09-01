# 31 — 单元测试框架 (M82, kernel 内断言自检)

状态: ✅ 完成。验收: QEMU 串口 `M82 RESULT: PASS` (7/7)。

## 1. 接口

| 编号 | 签名 | 说明 |
|------|------|------|
| 0x7901 | ut_run() | 运行全部注册用例; 返回 pass-fail |
| 0x7902 | ut_info(ptr) | u64×4: (pass, fail, total, allpass) |

## 2. 框架

- 注册表: `static CASES: [Option<fn() -> bool>; 8]`, `register()` 填槽;
- 启动 `utest::init()` 注册 7 个用例并打印套件就绪;
- 用例为纯函数 (无硬件/无静态断言面) — 保证内核任意状态可跑。

## 3. 用例 (7)

| 用例 | 断言面 |
|------|--------|
| tc_strlen | 手写长度 = 8 |
| tc_strcmp | 等/不等/前缀 区分 |
| tc_parse | hex 解析 1F/FF/0/10 |
| tc_math | 乘法/除法/取模一致性 |
| tc_strrev | 就地反转 abcd→dcba |
| tc_bits | count_ones/leading/trailing |
| tc_line_model | \n 行模型 (3 行, 首行 5) |

## 4. 实测 (m82_ut.elf)

```
ut   : PASS case (total 1..7)
ut   : run done pass=7 fail=0
m82: pass=00000007 fail=00000000 total=00000007
m82: M82 RESULT: PASS
```

## 5. 扩展

- 新模块函数用例 `register(fn)` 一行入套件;
- CI (fujoci) 已有 m82 断言行, 回退即触发。
