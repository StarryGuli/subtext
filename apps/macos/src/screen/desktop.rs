//! 桌面坐标的小工具：主屏高度、鼠标位置（CG 坐标）、窗口所属应用。

use objc2::MainThreadMarker;
use objc2_app_kit::{NSEvent, NSScreen};

use super::geometry;

/// 主屏高度：AppKit 与 CG 坐标互换要用。
pub(super) fn primary_height() -> f64 {
    MainThreadMarker::new()
        .and_then(|mtm| NSScreen::screens(mtm).firstObject())
        .map_or(0.0, |screen| screen.frame().size.height)
}

/// 鼠标位置，CG 坐标（原点在主屏左上）。
pub(super) fn mouse_cg() -> (f32, f32) {
    let mouse = NSEvent::mouseLocation();
    geometry::cocoa_to_cg(mouse.x, mouse.y, primary_height())
}

/// 窗口所属应用的 bundle id。
pub(super) fn bundle_of(pid: i32) -> Option<String> {
    objc2_app_kit::NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
        .and_then(|app| app.bundleIdentifier())
        .map(|id| id.to_string())
}
