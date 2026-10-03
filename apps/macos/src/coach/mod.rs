//! 双语教练的 macOS 一侧：触发（复制、上屏停顿）、面板、替换已上屏的文字。
//!
//! 闸门、提示词、后端与后台线程都在平台无关的 `subtext-coach`；这里只管把系统里发生的事变成请求、
//! 把结果画出来，以及按用户的点击改应用里的文字。

mod action;
mod attributed;
mod clipboard;
mod doc;
mod monitor;
mod panel;
mod preview;
mod replace;
mod show;
mod target;
mod trigger;
mod typed;

use std::time::Instant;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSRange, NSRect};
use subtext_coach::{CoachConfig, CoachOutput, CoachService, ConversationMemory, Mode};

pub use action::{CoachAction, MAX_OPTIONS};
pub use preview::run_if_requested as run_preview_if_requested;
pub use trigger::ComposeProbe;

use clipboard::ClipboardWatch;
use monitor::CoachMonitor;
use panel::CoachPanel;
use typed::TypedBuffer;

/// 面板上正显示的这一次教练。
struct Shown {
    id: u64,

    mode: Mode,

    /// 解码 / 改稿的原文，或组句时刚上屏的那句中文。
    source: String,

    output: Option<CoachOutput>,

    /// 失败时给用户看的原因。
    failure: Option<String>,

    /// 解码的译文是否已展开。
    revealed: bool,

    /// 面板跟着哪里出现：组句跟光标，解码跟鼠标（零矩形即取鼠标位置）。
    anchor: NSRect,

    /// 组句时能替换的那段已上屏文字；找不到就只能复制。
    target: Option<ReplaceTarget>,

    shown_at: Instant,

    /// 脚注里临时提示的一句话（替换失败改成了复制之类）。
    note: Option<String>,
}

/// 刚上屏的那句中文在应用里的位置。
pub(super) struct ReplaceTarget {
    client: Retained<AnyObject>,

    range: NSRange,

    /// 那段范围里应该是什么文字；替换前核对，对不上就不动应用里的内容。
    expected: String,
}

pub struct Coach {
    /// 当前套用的配置。
    config: CoachConfig,

    /// 开着才有。配置变了整个换掉，旧线程自己退出。
    service: Option<CoachService>,

    panel: CoachPanel,

    monitor: CoachMonitor,

    clipboard: ClipboardWatch,

    /// 刚解码过的对方消息，写回复时接它的语气。
    memory: ConversationMemory,

    /// 刚上屏的中文，攒成一句。
    typed: TypedBuffer,

    /// 最近一次上屏时候选窗口所在的光标矩形。
    last_anchor: NSRect,

    /// 最近一次上屏的客户端与应用。
    last_client: Option<Retained<AnyObject>>,

    last_app: Option<String>,

    next_id: u64,

    shown: Option<Shown>,

    /// 输入法正处于激活状态（轮询只在激活期间跑）。
    active: bool,
}

impl Coach {
    pub fn new(mtm: MainThreadMarker) -> Self {
        Self {
            config: CoachConfig::default(),
            service: None,
            panel: CoachPanel::new(mtm),
            monitor: CoachMonitor::new(mtm),
            clipboard: ClipboardWatch::new(),
            memory: ConversationMemory::default(),
            typed: TypedBuffer::default(),
            last_anchor: NSRect::ZERO,
            last_client: None,
            last_app: None,
            next_id: 0,
            shown: None,
            active: false,
        }
    }

    /// 套用配置：开关或后端设置变了才重建服务（重建会起新线程、丢缓存）。
    pub fn apply(&mut self, config: &CoachConfig) {
        if *config == self.config && (self.service.is_some() == config.enabled) {
            return;
        }
        self.config = config.clone();
        self.service = config.enabled.then(|| CoachService::start(config));
        if !config.enabled {
            self.dismiss();
            self.typed.reset();
        }
        self.sync_monitor();
        tracing::info!(enabled = config.enabled, backend = config.backend.key(), "双语教练配置已套用");
    }

    /// 输入法被切到前台：开始盯剪贴板与上屏停顿。此前复制的内容不追溯。
    pub fn activate(&mut self) {
        self.active = true;
        self.clipboard.reset();
        self.sync_monitor();
    }

    /// 输入法被切走：停止轮询，收起面板，丢掉没说完的一句。
    pub fn deactivate(&mut self) {
        self.active = false;
        self.typed.reset();
        self.dismiss();
        self.sync_monitor();
    }

    /// 关掉面板并忘掉正显示的内容。
    pub fn dismiss(&mut self) {
        self.panel.hide();
        self.shown = None;
    }

    fn sync_monitor(&mut self) {
        if self.active && self.service.is_some() {
            self.monitor.start();
        } else {
            self.monitor.stop();
        }
    }
}
