//! 屏幕上的窗口列表：`CGWindowListCopyWindowInfo`，按从前到后排。

use std::ffi::c_void;

use objc2::rc::Retained;
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString};
use subtext_coach::screen::Rect;

use super::geometry::WindowInfo;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    /// 返回的数组归调用方释放（Copy 规则）。
    fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> *const c_void;
}

/// 只要屏幕上看得见的。
const ON_SCREEN_ONLY: u32 = 1;

/// 不要桌面壁纸与桌面图标。
const EXCLUDE_DESKTOP_ELEMENTS: u32 = 16;

/// 当前屏幕上的窗口，从前到后。拿不到（没有权限、系统没给）返回空。
pub fn list() -> Vec<WindowInfo> {
    // SAFETY: 调用 CoreGraphics 的纯查询函数；返回值按 Copy 规则归本函数，下面用 from_raw 接管释放。
    let raw = unsafe { CGWindowListCopyWindowInfo(ON_SCREEN_ONLY | EXCLUDE_DESKTOP_ELEMENTS, 0) };
    if raw.is_null() {
        return Vec::new();
    }
    // SAFETY: CFArray 与 NSArray 免费桥接；元素是字典。
    let array: Option<Retained<NSArray>> = unsafe { Retained::from_raw(raw.cast_mut().cast()) };
    let Some(array) = array else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|item| item.downcast::<NSDictionary>().ok())
        .filter_map(|dict| parse(&dict))
        .collect()
}

/// 按窗口号找当前的位置与大小（窗口可能被拖动过）；窗口没了返回 `None`。
pub fn bounds_of(window_id: u32) -> Option<Rect> {
    list()
        .into_iter()
        .find(|window| window.id == window_id)
        .map(|window| window.bounds)
}

fn parse(dict: &NSDictionary) -> Option<WindowInfo> {
    let number = |key: &str| -> Option<f64> {
        let value = dict.objectForKey(&*NSString::from_str(key))?;
        value.downcast_ref::<NSNumber>().map(NSNumber::doubleValue)
    };
    let bounds = dict
        .objectForKey(&*NSString::from_str("kCGWindowBounds"))?
        .downcast::<NSDictionary>()
        .ok()?;
    let side = |key: &str| -> Option<f32> {
        let value = bounds.objectForKey(&*NSString::from_str(key))?;
        value
            .downcast_ref::<NSNumber>()
            .map(|n| n.doubleValue() as f32)
    };
    let owner = dict
        .objectForKey(&*NSString::from_str("kCGWindowOwnerName"))
        .and_then(|value| value.downcast::<NSString>().ok())
        .map(|name| name.to_string())
        .unwrap_or_default();
    Some(WindowInfo {
        id: number("kCGWindowNumber")? as u32,
        pid: number("kCGWindowOwnerPID")? as i32,
        owner,
        layer: number("kCGWindowLayer").unwrap_or(0.0) as i32,
        bounds: Rect::new(side("X")?, side("Y")?, side("Width")?, side("Height")?),
        alpha: number("kCGWindowAlpha").unwrap_or(1.0) as f32,
    })
}
