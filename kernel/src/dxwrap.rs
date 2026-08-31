//! dxwrap.rs — M56: DXVK 式翻译层原型 (D3D 命令模型 -> fujogl)
//!
//! 原型语义 (D3D 状态/顶点缓冲/变换 翻译到 fujogl 光栅命令):
//!   0x6301 dx_verts(ptr, n)    顶点缓冲拷贝 [(x,y)u32×3] (用户 → 内核)
//!   0x6302 dx_matrix(ptr)      仿射矩阵 4×u32 (sx, sy, tx, ty)
//!   0x6303 dx_flush(color)     变换顶点 -> gl 三角形光栅
//! 评估文档: docs/09-dxvk-feasibility.md (分层/缺口/路线)。

use crate::gl;
use crate::serial;

static mut DXV: [u32; 6] = [0; 6];
static mut DXM: [u32; 4] = [1, 1, 0, 0]; // sx, sy, tx, ty

/// 0x6301: 顶点缓冲 (6×u32).
pub fn fujo_dx_verts(ptr: u64, n: u64) -> i64 {
    unsafe {
        let cnt = (n as usize).min(6);
        for i in 0..cnt {
            DXV[i] = ((ptr as *const u32).add(i)).read();
        }
    }
    0
}

/// 0x6302: 仿射矩阵.
pub fn fujo_dx_matrix(ptr: u64) -> i64 {
    unsafe {
        for i in 0..4 {
            DXM[i] = ((ptr as *const u32).add(i)).read();
        }
    }
    0
}

/// 0x6303: flush — 变换 + 光栅.
pub fn fujo_dx_flush(color: u64) -> i64 {
    unsafe {
        let sx = DXM[0] as i64;
        let sy = DXM[1] as i64;
        let tx = DXM[2] as i64;
        let ty = DXM[3] as i64;
        let mut out = [0u32; 6];
        for i in 0..3 {
            let vx = DXV[i * 2] as i64;
            let vy = DXV[i * 2 + 1] as i64;
            out[i * 2] = (vx * sx + tx) as u32;
            out[i * 2 + 1] = (vy * sy + ty) as u32;
        }
        let _ = gl::fujo_gl_tri(out.as_ptr() as u64, color);
        serial::write_line("dxw : translated vertex stream -> fujogl raster");
    }
    0
}
