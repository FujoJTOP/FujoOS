/* fujokit.h — FujoOS 用户态控件库 v0 (M41)
 *
 * 零 libc, 纯几何/命中/状态机。控件由 (id, rect, 状态) 组成:
 *   kt_button   矩形命中+点击触发 (回调状态标志)
 *   kt_textbox  文本缓冲+追加/退格+光标
 *   kt_list     行表+命中选中
 * 渲染/消息环使用 fujo 原语 (fujokit 自身无 I/O; 宿主程序负责
 * font_text/wm 调用)。控件坐标直接注册为 wm 命中矩形 (z-order)。
 */
#ifndef FUJOKIT_H
#define FUJOKIT_H

typedef struct {
    int x, y, w, h;
} kt_rect;

/* ---- 按钮 ---- */
typedef struct {
    int id;
    kt_rect r;
    int pressed; /* 1 = 上一点击命中 (未释放) */
    int triggers; /* 累计点击次数 */
    char label[32];
} kt_button;

static int kt_rect_hit(kt_rect r, int x, int y)
{
    return x >= r.x && x <= r.x + r.w && y >= r.y && y <= r.y + r.h;
}

static void kt_button_init(kt_button *b, int id, int x, int y, int w, int h,
                           const char *label)
{
    b->id = id;
    b->r.x = x;
    b->r.y = y;
    b->r.w = w;
    b->r.h = h;
    b->pressed = 0;
    b->triggers = 0;
    {
        int i = 0;
        while (label[i] && i < 31) {
            b->label[i] = label[i];
            i++;
        }
        b->label[i] = 0;
    }
}

/* 点击消息 (按下+未释放) -> 触发; 返回触发事件 1/0 */
static int kt_button_click(kt_button *b, int x, int y, int down)
{
    if (down) {
        if (kt_rect_hit(b->r, x, y)) {
            b->triggers++;
            b->pressed = 1;
            return 1;
        }
    } else {
        b->pressed = 0;
    }
    return 0;
}

/* ---- 文本框 ---- */
typedef struct {
    int id;
    kt_rect r;
    char text[64];
    int len;
    int caret;
} kt_textbox;

static void kt_textbox_init(kt_textbox *t, int id, int x, int y, int w, int h)
{
    t->id = id;
    t->r.x = x;
    t->r.y = y;
    t->r.w = w;
    t->r.h = h;
    t->len = 0;
    t->caret = 0;
    t->text[0] = 0;
}

static int kt_textbox_append(kt_textbox *t, char ch)
{
    if (t->len < 63 && ch >= 32 && ch < 127) {
        t->text[t->len++] = ch;
        t->text[t->len] = 0;
        t->caret = t->len;
        return 1;
    }
    if (ch == 8 && t->len > 0) { /* backspace */
        t->len--;
        t->text[t->len] = 0;
        t->caret = t->len;
        return 1;
    }
    return 0;
}

/* ---- 列表 ---- */
typedef struct {
    int id;
    kt_rect r;
    char items[8][32];
    int count;
    int selected;
} kt_list;

static void kt_list_init(kt_list *l, int id, int x, int y, int w, int h)
{
    l->id = id;
    l->r.x = x;
    l->r.y = y;
    l->r.w = w;
    l->r.h = h;
    l->count = 0;
    l->selected = -1;
}

static void kt_list_add(kt_list *l, const char *item)
{
    if (l->count >= 8) {
        return;
    }
    int i = 0;
    while (item[i] && i < 31) {
        l->items[l->count][i] = item[i];
        i++;
    }
    l->items[l->count][i] = 0;
    l->count++;
}

/* 点击 -> 行命中 -> selected; 返回选中行号 (-1 无) */
static int kt_list_click(kt_list *l, int x, int y, int down)
{
    if (down && kt_rect_hit(l->r, x, y)) {
        int row = (y - l->r.y) / 12;
        if (row >= 0 && row < l->count) {
            l->selected = row;
            return row;
        }
    }
    return -1;
}

#endif /* FUJOKIT_H */
