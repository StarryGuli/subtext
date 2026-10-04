//! 渲染结果：位图加内容区（阴影边之外的那块）的位置。

use tiny_skia::Pixmap;

/// 一个可点击的候选格：第几行（对应 `Frame::rows` 的下标）与它在内容区里的矩形，单位是点，原点在内容区左上角。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub row: usize,

    pub x: f32,

    pub y: f32,

    pub width: f32,

    pub height: f32,
}

pub struct Rendered {
    /// 预乘 RGBA 位图，含阴影边。
    pub pixmap: Pixmap,

    /// 内容区左上角在位图里的像素坐标。
    pub content_x: u32,

    pub content_y: u32,

    /// 内容区像素宽高（窗口该有的大小）。
    pub content_width: u32,

    pub content_height: u32,

    /// 渲染用的倍数，壳把像素换回点用。
    pub scale: f32,

    /// 每个候选格的位置，壳按鼠标点击的位置找是哪一格。
    pub hits: Vec<Hit>,
}

impl Rendered {
    /// 内容区宽高换回点。
    pub fn content_size_points(&self) -> (f32, f32) {
        (
            self.content_width as f32 / self.scale,
            self.content_height as f32 / self.scale,
        )
    }
}
