//! 双语教练的 macOS 一侧：触发（复制、上屏停顿）、面板、替换已上屏的文字。
//!
//! 闸门、提示词、后端与后台线程都在平台无关的 `subtext-coach`；这里只管把系统里发生的事变成请求、
//! 把结果画出来，以及按用户的点击改应用里的文字。

mod action;
mod attributed;
mod clipboard;
mod connection;
mod doc;
mod monitor;
mod panel;
mod preview;
mod replace;
pub(crate) mod sentence;
mod show;
mod target;
mod trigger;
mod typed;

use std::collections::VecDeque;
use std::time::Instant;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSRange, NSRect};
use subtext_coach::{
    CoachConfig, CoachOutput, CoachService, ConversationMemory, Mode, SharedCache,
};

pub use action::CoachAction;
pub use doc::Doc;
pub use panel::{CoachPanel, PanelContent};
pub use preview::run_if_requested as run_preview_if_requested;
pub use trigger::{ComposeProbe, EnglishProbe, Probe};

use clipboard::ClipboardWatch;
use monitor::CoachMonitor;
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

    /// 还在流式生成：内容会继续变长，这时不能替换、复制。
    streaming: bool,

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

/// 面板上最多能翻回几条之前的解读。
const PAST_LIMIT: usize = 10;

/// 解读历史的文件名，在数据目录里。
const HISTORY_FILE: &str = "coach-history.jsonl";

pub struct Coach {
    /// 当前套用的配置。
    config: CoachConfig,

    /// 开着才有。配置变了整个换掉，旧线程自己退出。
    service: Option<CoachService>,

    /// 解读历史：解码、屏幕阅读预解码共用；换后端、开关教练重建服务时它不丢，`history` 开着时还落盘。
    cache: SharedCache,

    /// 现在的 `cache` 是不是按「落盘」建的。
    history_on: bool,

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

    /// 最近一次上屏英文的时间；停顿够久就去校对光标前那一句。
    english_dirty: Option<Instant>,

    /// 最近一次自动校对的那句话，同一句不重复弹。
    last_edit_text: Option<String>,

    next_id: u64,

    shown: Option<Shown>,

    /// 之前看过的解读，旧的在前；面板上可以翻回去看。只收已经出完结果的，替换位置不留（应用里的文字早就变了）。
    past: VecDeque<Shown>,

    /// 面板正在看 `past` 里的第几条；`None` 是最新的那条。
    view: Option<usize>,

    /// 系统当前选中的输入源是不是言外。轮询只在它为真时处理复制与上屏；
    /// 不跟 IMK 的激活回调走，那个随文本框焦点来回跳（复制网页上的文字时根本没有文本框）。
    source_ours: bool,

    /// 上次问系统当前输入源的时间，一秒问一次。
    source_checked: Instant,

    /// 进行中的「测试连接」，结果是给设置页底部状态行的一句话。
    test: Option<std::sync::mpsc::Receiver<String>>,
}

impl Coach {
    pub fn new(mtm: MainThreadMarker) -> Self {
        Self {
            config: CoachConfig::default(),
            service: None,
            cache: SharedCache::in_memory(),
            history_on: false,
            panel: CoachPanel::new(mtm),
            monitor: CoachMonitor::new(mtm),
            clipboard: ClipboardWatch::new(),
            memory: ConversationMemory::default(),
            typed: TypedBuffer::default(),
            last_anchor: NSRect::ZERO,
            last_client: None,
            last_app: None,
            english_dirty: None,
            last_edit_text: None,
            next_id: 0,
            shown: None,
            past: VecDeque::new(),
            view: None,
            source_ours: false,
            source_checked: Instant::now(),
            test: None,
        }
    }

    /// 套用配置：开关或后端设置变了才重建服务（重建会起新线程、丢缓存）。
    pub fn apply(&mut self, config: &CoachConfig) {
        self.sync_cache(config.history);
        if *config == self.config && (self.service.is_some() == config.enabled) {
            return;
        }
        self.config = config.clone();
        self.service = config
            .enabled
            .then(|| CoachService::start_with_cache(config, self.cache.clone()));
        if !config.enabled {
            self.dismiss();
            self.past.clear();
            self.typed.reset();
        }
        self.sync_monitor();
        tracing::info!(
            enabled = config.enabled,
            backend = config.backend.key(),
            "双语教练配置已套用"
        );
    }

    /// 解读历史的共享缓存：屏幕阅读也用它，复制同一段文字时才会命中预解码的结果。
    pub fn cache(&self) -> SharedCache {
        self.cache.clone()
    }

    /// 清除解读历史（内存与磁盘）。
    pub fn clear_history(&self) {
        self.cache.clear();
    }

    /// `history` 开关变了就换一份缓存：开着落盘到数据目录，关着只放内存并删掉磁盘上的文件。
    fn sync_cache(&mut self, history: bool) {
        if history == self.history_on {
            return;
        }
        self.history_on = history;
        let path = crate::app::paths::user_data_dir().map(|dir| dir.join(HISTORY_FILE));
        self.cache = match (history, path) {
            (true, Some(path)) => SharedCache::persistent(path),
            (false, Some(path)) => {
                let _ = std::fs::remove_file(path);
                SharedCache::in_memory()
            }
            (_, None) => SharedCache::in_memory(),
        };
    }

    /// 某个文本框把输入法激活了：此刻当前输入源一定是言外。此前复制的内容不追溯。
    pub fn activate(&mut self) {
        if !self.source_ours {
            self.clipboard.reset();
        }
        self.source_ours = true;
        self.source_checked = Instant::now();
    }

    /// 某个文本框失焦：只丢掉没说完的那一句。面板与轮询不动，焦点来回跳不该让正在读的结果消失。
    pub fn deactivate(&mut self) {
        self.typed.reset();
        self.english_dirty = None;
    }

    /// 问系统当前输入源是不是言外（一秒一次）。切走就收起面板、丢掉没写完的内容，切回来从当前剪贴板重新算起。
    pub(super) fn refresh_source(&mut self) {
        if self.source_checked.elapsed() < std::time::Duration::from_secs(1) {
            return;
        }
        self.source_checked = Instant::now();
        let ours = crate::app::input_source::current_is_ours();
        if ours && !self.source_ours {
            self.clipboard.reset();
        }
        if !ours && self.source_ours {
            self.typed.reset();
            self.english_dirty = None;
            self.dismiss();
        }
        self.source_ours = ours;
    }

    /// 教练是否开着。
    pub fn is_enabled(&self) -> bool {
        self.service.is_some()
    }

    /// 关掉面板并忘掉正显示的内容。
    pub fn dismiss(&mut self) {
        self.panel.hide();
        if let Some(shown) = self.shown.take() {
            self.archive(shown);
        }
        self.view = None;
    }

    /// 把出完结果的一条收进「之前的解读」；没出完的（被新请求顶掉、失败）不收。
    fn archive(&mut self, mut shown: Shown) {
        if shown.output.is_none() || shown.failure.is_some() || shown.streaming {
            return;
        }
        shown.target = None;
        shown.note = None;
        self.past.push_back(shown);
        while self.past.len() > PAST_LIMIT {
            self.past.pop_front();
        }
    }

    fn sync_monitor(&mut self) {
        // 轮询在教练开着（或正在测试连接）时一直跑；是不是该处理由每次轮询里问系统当前输入源决定
        if self.service.is_some() || self.test.is_some() {
            self.monitor.start();
        } else {
            self.monitor.stop();
        }
    }
}
