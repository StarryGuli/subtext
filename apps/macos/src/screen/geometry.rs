//! 屏幕坐标的换算与窗口选择，纯逻辑，不碰 AppKit。
//!
//! 约定：CG 坐标系，单位是点，原点在主屏左上角、y 向下（`CGWindowListCopyWindowInfo`、`screencapture -R` 都用它）。
//! AppKit 的鼠标位置原点在主屏左下角、y 向上，进来先换成 CG。

use subtext_coach::screen::{OcrLine, Rect};

/// 屏幕上的一个窗口。
#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    pub id: u32,

    pub pid: i32,

    pub owner: String,

    /// 0 是普通窗口；菜单栏、程序坞、浮层都在更高的层。
    pub layer: i32,

    pub bounds: Rect,

    pub alpha: f32,
}

/// 窗口小于这个边长（点）不当作阅读目标：多半是按钮、角标之类的小浮层。
const MIN_WINDOW_SIDE: f32 = 120.0;

/// 鼠标（CG 坐标）下面最靠前的普通窗口。`windows` 按从前到后排；跳过自己进程的窗口（候选窗、教练面板、提示条）。
pub fn pick_window(windows: &[WindowInfo], x: f32, y: f32, own_pid: i32) -> Option<&WindowInfo> {
    windows.iter().find(|window| {
        window.layer == 0
            && window.pid != own_pid
            && window.alpha > 0.0
            && window.bounds.width >= MIN_WINDOW_SIDE
            && window.bounds.height >= MIN_WINDOW_SIDE
            && window.bounds.contains(x, y, 0.0)
    })
}

/// OCR 给的一行：比例坐标，原点在图片左上角。
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedLine {
    pub text: String,

    pub x: f32,

    pub y: f32,

    pub width: f32,

    pub height: f32,
}

/// 比例坐标换成屏幕点：图片对应屏幕上的 `bounds`。
pub fn to_screen_lines(lines: &[NormalizedLine], bounds: Rect) -> Vec<OcrLine> {
    lines
        .iter()
        .map(|line| OcrLine {
            text: line.text.clone(),
            rect: Rect::new(
                bounds.x + line.x * bounds.width,
                bounds.y + line.y * bounds.height,
                line.width * bounds.width,
                line.height * bounds.height,
            ),
        })
        .collect()
}

/// AppKit 鼠标位置（原点在主屏左下）换成 CG 坐标（原点在主屏左上）。
pub fn cocoa_to_cg(x: f64, y: f64, primary_height: f64) -> (f32, f32) {
    (x as f32, (primary_height - y) as f32)
}

/// CG 矩形换成 AppKit 矩形的左下角 y（面板定位用）。
pub fn cg_to_cocoa_y(y: f32, height: f32, primary_height: f64) -> f64 {
    primary_height - f64::from(y) - f64::from(height)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(id: u32, pid: i32, layer: i32, x: f32, y: f32, w: f32, h: f32) -> WindowInfo {
        WindowInfo {
            id,
            pid,
            owner: format!("app{pid}"),
            layer,
            bounds: Rect::new(x, y, w, h),
            alpha: 1.0,
        }
    }

    #[test]
    fn picks_the_frontmost_normal_window_under_the_pointer() {
        let windows = [
            window(1, 10, 25, 0.0, 0.0, 1440.0, 24.0),    // 菜单栏
            window(2, 99, 0, 200.0, 200.0, 500.0, 400.0), // 自己的面板
            window(3, 20, 0, 100.0, 100.0, 800.0, 600.0), // 前面的聊天窗口
            window(4, 30, 0, 0.0, 0.0, 1440.0, 900.0),    // 后面铺满的窗口
        ];
        assert_eq!(pick_window(&windows, 300.0, 300.0, 99).unwrap().id, 3);
        assert_eq!(pick_window(&windows, 1000.0, 800.0, 99).unwrap().id, 4);
        assert!(pick_window(&windows, 5000.0, 5000.0, 99).is_none());
    }

    #[test]
    fn tiny_and_transparent_windows_are_ignored() {
        let mut ghost = window(1, 10, 0, 0.0, 0.0, 800.0, 600.0);
        ghost.alpha = 0.0;
        let windows = [
            ghost,
            window(2, 11, 0, 0.0, 0.0, 60.0, 60.0),
            window(3, 12, 0, 0.0, 0.0, 700.0, 500.0),
        ];
        assert_eq!(pick_window(&windows, 30.0, 30.0, 99).unwrap().id, 3);
    }

    #[test]
    fn normalized_lines_land_inside_the_window() {
        let lines = [NormalizedLine {
            text: "hello".to_owned(),
            x: 0.1,
            y: 0.5,
            width: 0.25,
            height: 0.05,
        }];
        let mapped = to_screen_lines(&lines, Rect::new(100.0, 200.0, 800.0, 600.0));
        assert_eq!(mapped[0].rect, Rect::new(180.0, 500.0, 200.0, 30.0));
    }

    #[test]
    fn coordinate_flips_are_inverses() {
        let (x, y) = cocoa_to_cg(120.0, 700.0, 900.0);
        assert_eq!((x, y), (120.0, 200.0));
        // 屏幕上 CG 的 (y=200, 高 30) 那条，换回 AppKit 的左下角 y 是 670
        assert_eq!(cg_to_cocoa_y(200.0, 30.0, 900.0), 670.0);
    }
}
