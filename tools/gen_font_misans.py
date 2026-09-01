#!/usr/bin/env python3
"""gen_font_misans.py — 用 MiSans (小米字体) 渲染 8x12 位图字模 (0x20..0x7F)。

MiSans-Regular 是比例字体; 以固定 cell (9x14 渲染 -> 8x12 阈值) 归一化,
每字符 12 字节, 每字节 = 1 行 8 列 (bit7=最左列)。用于桌面/窗口/任务栏
全部文字 —— 统一 MiSans 风格 (M110)。
"""
from PIL import Image, ImageDraw, ImageFont
import os

FONT_PATH = os.path.join(os.path.dirname(__file__), '..', 'sdk', 'fonts', 'MiSans-Regular.ttf')
FONT_SIZE = 14  # 8x12 cell 对应字号 (MiSans 12px 笔画偏细, 用 14px)

GLYPHS = []
for code in range(0x20, 0x7F):
    ch = chr(code)
    img = Image.new('L', (12, 18), 255)
    d = ImageDraw.Draw(img)
    d.text((2, 2), ch, font=ImageFont.truetype(FONT_PATH, FONT_SIZE), fill=0)
    px = img.load()
    # 找包围盒
    minx, maxx, miny, maxy = 12, -1, 18, -1
    for y in range(18):
        for x in range(12):
            if px[x, y] < 128:
                minx = min(minx, x); maxx = max(maxx, x)
                miny = min(miny, y); maxy = max(maxy, y)
    # 裁 8 列 x 11 行 (有效字形区; 最后 1 行留给 AA/下延): 水平居中
    w_used = maxx - minx + 1
    x_off = minx + (w_used - 8) // 2
    y_off = miny - 2
    if y_off < 0:
        y_off = 0
    rows = []
    for y in range(11):
        bits = 0
        for x in range(8):
            sx = x_off + x
            sy = y_off + y
            if 0 <= sx < 12 and 0 <= sy < 18 and px[sx, sy] < 128:
                bits |= (1 << (7 - x))
        rows.append(bits)
    GLYPHS.append(rows)

# 0x7F DEL 保留
GLYPHS.append([0] * 12)

lines = []
for i, g in enumerate(GLYPHS):
    ch = chr(0x20 + i)
    lines.append('    [%s], // 0x%02X %r' % (', '.join('0x%02X' % b for b in g), 0x20 + i, ch))
print('\n'.join(lines))
