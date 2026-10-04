//! 截屏：调系统自带的 `screencapture`，要「屏幕录制」权限（授给 Subtext）。

use std::path::Path;
use std::process::{Command, Stdio};

use subtext_coach::screen::Rect;

use super::target::Target;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    /// 当前进程有没有屏幕录制权限；不弹窗。
    fn CGPreflightScreenCaptureAccess() -> bool;

    /// 请求屏幕录制权限：第一次会弹系统对话框，返回当前是否已授权。
    fn CGRequestScreenCaptureAccess() -> bool;
}

/// 有没有屏幕录制权限。
pub fn has_permission() -> bool {
    // SAFETY: 无参数的纯查询函数。
    unsafe { CGPreflightScreenCaptureAccess() }
}

/// 请求权限（会弹系统对话框，只在用户明确要开始屏幕阅读时调）。
pub fn request_permission() -> bool {
    // SAFETY: 无参数；第一次调用弹系统对话框。
    unsafe { CGRequestScreenCaptureAccess() }
}

/// 把 `target` 截成 PNG 写到 `path`。窗口按窗口号截，被别的窗口挡住的部分也能截到；区域按屏幕坐标截。
pub fn capture(target: &Target, bounds: Rect, path: &Path) -> Result<(), String> {
    let mut command = Command::new("/usr/sbin/screencapture");
    // -x 不出快门声，-o 窗口不带阴影
    command.args(["-x", "-o"]);
    match target {
        Target::Window { id, .. } => {
            command.arg(format!("-l{id}"));
        }
        Target::Region { .. } => {
            command.arg(format!(
                "-R{},{},{},{}",
                bounds.x.round() as i64,
                bounds.y.round() as i64,
                bounds.width.round() as i64,
                bounds.height.round() as i64
            ));
        }
    }
    let status = command
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("起不了 screencapture：{error}"))?;
    let written = path.metadata().map(|meta| meta.len()).unwrap_or(0);
    if status.success() && written > 0 {
        Ok(())
    } else {
        Err("截屏失败，多半是还没授予屏幕录制权限".to_owned())
    }
}
