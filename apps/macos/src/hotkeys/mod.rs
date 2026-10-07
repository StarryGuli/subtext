//! 全局热键（Carbon `RegisterEventHotKey`）：⌘⇧ 这类组合键会先被应用自己的菜单快捷键截走，输入法收不到，
//! 同一个键在不同应用里时灵时不灵；系统级热键不经过应用菜单，处处一致。
//!
//! 只在言外是当前输入法、教练开着时注册，切走就注销：否则别的输入法下 ⌘⇧R 刷新网页也会被吞掉。

mod keymap;

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::null_mut;

use subtext_platform::KeyCombo;

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

    fn UnregisterEventHotKey(hotkey: *mut c_void) -> OsStatus;

    fn GetEventParameter(
        event: *mut c_void,
        name: u32,
        desired_type: u32,
        actual_type: *mut u32,
        size: usize,
        actual_size: *mut usize,
        data: *mut c_void,
    ) -> OsStatus;
}

/// 'keyb'
const EVENT_CLASS_KEYBOARD: u32 = 0x6B65_7962;
const EVENT_HOT_KEY_PRESSED: u32 = 5;
/// '----'
const EVENT_PARAM_DIRECT_OBJECT: u32 = 0x2D2D_2D2D;
/// 'hkid'
const TYPE_EVENT_HOT_KEY_ID: u32 = 0x686B_6964;
/// 'SBTX'
const SIGNATURE: u32 = 0x5342_5458;

/// 热键编号：解析剪贴板、开始 / 停止屏幕阅读。
const CLIPBOARD: u32 = 1;
const SCREEN: u32 = 2;

#[derive(Default)]
struct State {
    handler_installed: bool,

    registered: Vec<*mut c_void>,

    /// 当前注册的那组键位；`None` 是没注册。
    wanted: Option<(KeyCombo, KeyCombo)>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// 让注册状态跟上需要：`active` 为真注册这两个键，否则注销。键位没变就什么都不做，可以每次轮询都调。
pub fn sync(active: bool, clipboard: KeyCombo, screen: KeyCombo) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let wanted = active.then_some((clipboard, screen));
        if state.wanted == wanted {
            return;
        }
        for hotkey in state.registered.drain(..) {
            // SAFETY: hotkey 是 RegisterEventHotKey 返回的引用，只注销一次
            unsafe { UnregisterEventHotKey(hotkey) };
        }
        state.wanted = wanted;
        let Some((clipboard, screen)) = wanted else {
            return;
        };
        if !state.handler_installed {
            let spec = EventTypeSpec {
                event_class: EVENT_CLASS_KEYBOARD,
                event_kind: EVENT_HOT_KEY_PRESSED,
            };
            let mut handler_ref = null_mut();
            // SAFETY: spec 在调用期间有效；handler 是 'static 的 extern "C" 函数；输出参数指向本地变量
            let status = unsafe {
                InstallEventHandler(
                    GetApplicationEventTarget(),
                    on_hotkey,
                    1,
                    &raw const spec,
                    null_mut(),
                    &raw mut handler_ref,
                )
            };
            if status != 0 {
                tracing::warn!(status, "全局热键处理器装不上，⌘⇧ 组合键退回输入法内处理");
                return;
            }
            state.handler_installed = true;
        }
        for (id, combo) in [(CLIPBOARD, clipboard), (SCREEN, screen)] {
            let Some((code, modifiers)) = keymap::carbon(combo) else {
                tracing::debug!(%combo, "这个键位不注册成全局热键");
                continue;
            };
            let mut hotkey = null_mut();
            // SAFETY: 参数都是普通值，输出参数指向本地变量
            let status = unsafe {
                RegisterEventHotKey(
                    code,
                    modifiers,
                    EventHotKeyId {
                        signature: SIGNATURE,
                        id,
                    },
                    GetApplicationEventTarget(),
                    0,
                    &raw mut hotkey,
                )
            };
            if status == 0 {
                state.registered.push(hotkey);
            } else {
                tracing::warn!(%combo, status, "全局热键注册失败（可能被别的程序占了）");
            }
        }
    });
}

/// 系统把热键事件送到这里（主线程）。
extern "C" fn on_hotkey(_next: *mut c_void, event: *mut c_void, _user: *mut c_void) -> OsStatus {
    let mut id = EventHotKeyId {
        signature: 0,
        id: 0,
    };
    // SAFETY: event 由系统传入、回调期间有效；data 指向与 size 相符的本地结构
    let status = unsafe {
        GetEventParameter(
            event,
            EVENT_PARAM_DIRECT_OBJECT,
            TYPE_EVENT_HOT_KEY_ID,
            null_mut(),
            size_of::<EventHotKeyId>(),
            null_mut(),
            (&raw mut id).cast(),
        )
    };
    if status == 0 && id.signature == SIGNATURE {
        // 回调不能让 panic 穿过 C 栈
        let _ = std::panic::catch_unwind(|| match id.id {
            CLIPBOARD => crate::host::coach_clipboard_now(),
            SCREEN => crate::host::screen_toggle(),
            _ => {}
        });
    }
    0
}
