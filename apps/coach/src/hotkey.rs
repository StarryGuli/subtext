//! 全局热键（Carbon `RegisterEventHotKey`）：系统级，不经过应用菜单，任何应用在前台都灵。

use std::ffi::c_void;
use std::ptr::null_mut;

type OsStatus = i32;
type Handler = extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> OsStatus;

#[repr(C)]
struct EventTypeSpec {
    event_class: u32,
    event_kind: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct EventHotKeyId {
    signature: u32,
    id: u32,
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn GetApplicationEventTarget() -> *mut c_void;
    fn InstallEventHandler(
        target: *mut c_void,
        handler: Handler,
        count: u32,
        list: *const EventTypeSpec,
        user_data: *mut c_void,
        out: *mut *mut c_void,
    ) -> OsStatus;
    fn RegisterEventHotKey(
        code: u32,
        modifiers: u32,
        id: EventHotKeyId,
        target: *mut c_void,
        options: u32,
        out: *mut *mut c_void,
    ) -> OsStatus;
}

/// 'keyb'
const EVENT_CLASS_KEYBOARD: u32 = 0x6B65_7962;
const EVENT_HOT_KEY_PRESSED: u32 = 5;
/// 'SBTX'
const SIGNATURE: u32 = 0x5342_5458;

/// 空格的虚拟键码，与 ⌃⌥ 组合：Carbon 的 controlKey = 0x1000、optionKey = 0x0800。
const KEY_SPACE: u32 = 49;
const MODIFIERS: u32 = 0x1000 | 0x0800;

/// 快捷键的显示名。
pub const LABEL: &str = "⌃⌥Space";

/// 注册 ⌃⌥Space；按下时在主线程调 `on_press`。失败（被别的程序占了）返回 `false`。
pub fn register(on_press: fn()) -> bool {
    CALLBACK.with(|cell| cell.set(Some(on_press)));
    let spec = EventTypeSpec {
        event_class: EVENT_CLASS_KEYBOARD,
        event_kind: EVENT_HOT_KEY_PRESSED,
    };
    let mut handler_ref = null_mut();
    // SAFETY: 参数都是普通值，输出参数指向本地变量；处理器是不捕获状态的 extern "C" 函数
    let installed = unsafe {
        InstallEventHandler(
            GetApplicationEventTarget(),
            on_hotkey,
            1,
            &raw const spec,
            null_mut(),
            &raw mut handler_ref,
        )
    };
    if installed != 0 {
        tracing::warn!(installed, "全局热键处理器装不上");
        return false;
    }
    let mut hotkey = null_mut();
    // SAFETY: 同上
    let status = unsafe {
        RegisterEventHotKey(
            KEY_SPACE,
            MODIFIERS,
            EventHotKeyId {
                signature: SIGNATURE,
                id: 1,
            },
            GetApplicationEventTarget(),
            0,
            &raw mut hotkey,
        )
    };
    if status != 0 {
        tracing::warn!(status, "全局热键注册失败（可能被别的程序占了）");
    }
    status == 0
}

thread_local! {
    static CALLBACK: std::cell::Cell<Option<fn()>> = const { std::cell::Cell::new(None) };
}

extern "C" fn on_hotkey(_next: *mut c_void, _event: *mut c_void, _user: *mut c_void) -> OsStatus {
    CALLBACK.with(|cell| {
        if let Some(callback) = cell.get() {
            callback();
        }
    });
    0
}
