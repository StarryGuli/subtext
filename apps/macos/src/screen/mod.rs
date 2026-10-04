//! 屏幕阅读：持续截取指定窗口或区域、本机 OCR，新增的英文自动翻译，鼠标停在哪句就解释哪句。
//!
//! 截屏、OCR、窗口列表、悬浮卡在这里（平台相关）；块合并、去重记忆、提示词、后端在 `subtext-coach`。

mod active;
pub mod capture;
mod desktop;
pub mod geometry;
mod monitor;
pub mod ocr;
mod peek;
mod reader;
mod region;
pub mod scan;
mod selftest;
pub mod target;
pub mod windows;

pub use reader::ScreenReader;
pub use selftest::run_if_requested as run_selftest_if_requested;

/// 开发探测：`subtext-macos --screen-probe`。打印屏幕录制权限与当前窗口列表，看窗口枚举、权限检查在这台机器上通不通。
pub fn run_probe_if_requested() -> Option<i32> {
    if !std::env::args().any(|argument| argument == "--screen-probe") {
        return None;
    }
    println!("屏幕录制权限：{}", capture::has_permission());
    println!("OCR 程序：{:?}", ocr::helper_path());
    for window in windows::list().iter().take(12) {
        println!(
            "  #{} pid={} layer={} {:>4}x{:<4} @({:.0},{:.0}) {}",
            window.id,
            window.pid,
            window.layer,
            window.bounds.width,
            window.bounds.height,
            window.bounds.x,
            window.bounds.y,
            window.owner
        );
    }
    Some(0)
}
