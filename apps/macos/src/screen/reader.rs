//! 屏幕阅读的状态机：调度后台扫描、把 OCR 的行合并成消息块、新出现的英文块批量交给后端翻译、
//! 鼠标停在哪一块就在旁边弹出那一块的译文与提示。

use std::collections::HashMap;
use std::time::{Duration, Instant};

use objc2::MainThreadMarker;
use objc2_app_kit::{NSEvent, NSScreen};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use subtext_coach::gate::Gate;
use subtext_coach::screen::{Analysis, ScreenBlock, ScreenMemory, block_at, group_blocks};
use subtext_coach::{
    CoachConfig, CoachContext, CoachEvent, CoachOutput, CoachRequest, CoachService, Mode,
};

use super::monitor::ScreenMonitor;
use super::region::RegionPicker;
use super::scan::{ScanJob, ScanOutcome, Scanner};
use super::target::Target;
use super::{capture, geometry, ocr, windows};
use crate::coach::{CoachPanel, Doc, PanelContent};

/// 悬浮卡宽度、状态条宽度。
const CARD_WIDTH: f64 = 340.0;
const PILL_WIDTH: f64 = 230.0;

/// 鼠标在一块上停多久才弹出悬浮卡：划过去不弹。
const HOVER_DELAY: Duration = Duration::from_millis(350);

/// 鼠标离块边缘这么近（点）也算指着它。
const HOVER_SLACK: f32 = 3.0;

/// 一次最多翻译几块；多了留最新的，旧的等下一批。
const BATCH: usize = 5;

/// 翻译失败后隔多久再试。
const RETRY_DELAY: Duration = Duration::from_secs(8);

/// 给模型看的上文最多几块。
const CONTEXT_BLOCKS: usize = 6;

/// 同时最多几个翻译请求在飞：一个后台批量，一个鼠标指着的那块插队。
const MAX_IN_FLIGHT: usize = 2;

/// 阅读中的一切；停下来就整个丢掉（含识别过的屏幕文字）。
pub(super) struct Active {
    target: Target,

    helper: std::path::PathBuf,

    scanner: Scanner,

    /// 屏幕阅读自己的后端线程：不和输入时的解码 / 组句抢同一个队列。
    service: CoachService,

    gate: Gate,

    interval: Duration,

    memory: ScreenMemory,

    blocks: Vec<ScreenBlock>,

    /// 在飞的请求：编号 → 它负责的块标识。
    in_flight: HashMap<u64, Vec<String>>,

    next_id: u64,

    last_scan: Instant,

    last_error: Option<String>,

    retry_after: Instant,

    hover_key: Option<String>,

    hover_since: Instant,

    /// 悬浮卡正显示的内容：(块标识, 有无译文, 是否在翻译)，变了才重画。
    card_state: Option<(String, bool, bool)>,

    last_pill: Instant,

    /// 已经拿到译文的块数，状态条上显示。
    translated: usize,

    /// 系统正在输入密码：暂停中。
    paused: bool,

    /// 目标此刻在屏幕上的位置：窗口会被拖动，状态条跟着它走。
    bounds: subtext_coach::screen::Rect,

    /// 自测用：不真的截屏。
    fake_image: Option<std::path::PathBuf>,
}

pub struct ScreenReader {
    config: CoachConfig,

    monitor: ScreenMonitor,

    card: CoachPanel,

    pill: CoachPanel,

    active: Option<Active>,

    /// 菜单「读取鼠标所在窗口」选了之后，等到这个时间再取鼠标下的窗口（给用户时间把鼠标移过去）。
    pick_at: Option<Instant>,

    /// 要告诉用户的一句话，由宿主取走显示。
    notice: Option<String>,

    /// 正在框选区域时的选框层。
    picker: Option<RegionPicker>,

    /// 选框层交回的结果：`Some(None)` 是取消，`Some(Some(区域))` 是选好了；下一次轮询里处理。
    region_result: Option<Option<subtext_coach::screen::Rect>>,

    mtm: MainThreadMarker,
}

impl ScreenReader {
    pub fn new(mtm: MainThreadMarker) -> Self {
        let card = CoachPanel::with_width(mtm, CARD_WIDTH);
        card.set_ignores_mouse(true);
        let pill = CoachPanel::with_width(mtm, PILL_WIDTH);
        pill.set_ignores_mouse(true);
        Self {
            config: CoachConfig::default(),
            monitor: ScreenMonitor::new(mtm),
            card,
            pill,
            active: None,
            pick_at: None,
            notice: None,
            picker: None,
            region_result: None,
            mtm,
        }
    }

    pub fn apply(&mut self, config: &CoachConfig) {
        self.config = config.clone();
        if !config.enabled {
            self.stop();
        }
    }

    pub fn is_reading(&self) -> bool {
        self.active.is_some()
    }

    pub fn take_notice(&mut self) -> Option<String> {
        self.notice.take()
    }

    /// 几秒后取鼠标所在的窗口开始阅读：菜单点完会收起，要给用户时间把鼠标移到目标窗口上。
    pub fn schedule_window_pick(&mut self, delay: Duration) {
        self.pick_at = Some(Instant::now() + delay);
        self.monitor.start();
    }

    /// 开始框选：盖上选框层，选完在下一次轮询里开始读。
    pub fn begin_region_pick(&mut self) {
        self.picker = Some(RegionPicker::begin(self.mtm));
        self.monitor.start();
        self.notice = Some("拖出要读的区域；单击取消".to_owned());
    }

    /// 选框层交回结果（在鼠标事件里调，所以只记下，不在这里销毁选框层）。
    pub fn region_done(&mut self, region: Option<subtext_coach::screen::Rect>) {
        self.region_result = Some(region);
    }

    /// 取鼠标所在的窗口开始阅读。
    pub fn start_window_under_mouse(&mut self) -> Result<(), String> {
        let (x, y) = mouse_cg();
        let own_pid = std::process::id() as i32;
        let all = windows::list();
        let Some(window) = geometry::pick_window(&all, x, y, own_pid) else {
            return Err("鼠标下面没有可读取的窗口".to_owned());
        };
        let bundle = bundle_of(window.pid);
        self.start(Target::Window {
            id: window.id,
            owner: window.owner.clone(),
            bundle,
            bounds: window.bounds,
        })
    }

    /// 开始阅读 `target`。教练没开、没权限、没有 OCR 程序都会返回要告诉用户的话。
    pub fn start(&mut self, target: Target) -> Result<(), String> {
        if !self.config.enabled {
            return Err("请先在偏好设置里启用双语教练".to_owned());
        }
        if !capture::has_permission() {
            capture::request_permission();
            return Err(
                "屏幕阅读需要「屏幕录制」权限：请在 系统设置 → 隐私与安全性 → 屏幕录制 里允许 Subtext，授权后重新开始（可能需要先注销再登录）"
                    .to_owned(),
            );
        }
        let Some(helper) = ocr::helper_path() else {
            return Err("找不到 OCR 程序 subtext-ocr，请重新安装".to_owned());
        };
        self.stop();
        let label = target.label();
        self.active = Some(Active::new(target, helper, &self.config));
        self.monitor.start();
        self.notice = Some(format!("开始屏幕阅读：{label}。鼠标停在英文上就会显示译文"));
        Ok(())
    }

    /// 停止：丢掉识别过的屏幕文字，收起悬浮卡与状态条。
    pub fn stop(&mut self) {
        let was_reading = self.active.take().is_some();
        self.pick_at = None;
        self.monitor.stop();
        self.card.hide();
        self.pill.hide();
        if was_reading {
            self.notice = Some("已停止屏幕阅读".to_owned());
        }
    }

    /// 每 0.12 秒一次。
    pub fn tick(&mut self) {
        if let Some(result) = self.region_result.take() {
            self.picker = None;
            match result {
                Some(bounds) => {
                    if let Err(message) = self.start(Target::Region { bounds }) {
                        self.notice = Some(message);
                    }
                }
                None => self.notice = Some("已取消框选".to_owned()),
            }
            if self.active.is_none() && self.pick_at.is_none() {
                self.monitor.stop();
            }
        }
        if let Some(due) = self.pick_at
            && Instant::now() >= due
        {
            self.pick_at = None;
            if let Err(message) = self.start_window_under_mouse() {
                self.notice = Some(message);
                if self.active.is_none() {
                    self.monitor.stop();
                }
            }
        }
        let Some(active) = self.active.as_mut() else {
            return;
        };
        // 系统在输入密码（Secure Input）：暂停读取，什么都不截、不发，悬浮卡也收起
        if crate::imk::secure_input::enabled() {
            active.paused = true;
            self.card.hide();
            active.update_pill(&mut self.pill);
            return;
        }
        active.paused = false;
        let mut lost = false;
        active.drive(&mut lost);
        if lost {
            self.notice = Some("阅读的窗口不见了，已停止屏幕阅读".to_owned());
            self.stop();
            return;
        }
        active.update_card(&mut self.card);
        active.update_pill(&mut self.pill);
    }
}

impl Active {
    pub(super) fn new(target: Target, helper: std::path::PathBuf, config: &CoachConfig) -> Self {
        let target_bounds = target.bounds();
        Self {
            target,
            helper,
            scanner: Scanner::start(),
            service: CoachService::start(config),
            gate: Gate::new(config.max_chars, config.skip_apps.clone()),
            interval: Duration::from_millis(config.screen_interval_ms.max(500)),
            memory: ScreenMemory::default(),
            blocks: Vec::new(),
            in_flight: HashMap::new(),
            next_id: 0,
            last_scan: Instant::now() - Duration::from_secs(60),
            last_error: None,
            retry_after: Instant::now(),
            hover_key: None,
            hover_since: Instant::now(),
            card_state: None,
            last_pill: Instant::now() - Duration::from_secs(60),
            translated: 0,
            paused: false,
            bounds: target_bounds,
            fake_image: None,
        }
    }

    /// 自测用：当前识别出的块与它们的译文。
    pub(super) fn snapshot(&self) -> &[ScreenBlock] {
        &self.blocks
    }

    pub(super) fn set_fake_image(&mut self, image: std::path::PathBuf) {
        self.fake_image = Some(image);
    }

    pub(super) fn drive_once(&mut self) -> bool {
        let mut lost = false;
        self.drive(&mut lost);
        lost
    }

    /// 取扫描结果、按时发起新扫描、处理后端事件、提交新的翻译。
    fn drive(&mut self, lost: &mut bool) {
        while let Some(outcome) = self.scanner.poll() {
            match outcome {
                ScanOutcome::Lines { lines, bounds } => {
                    self.bounds = bounds;
                    self.last_error = None;
                    let blocks = group_blocks(&lines);
                    self.blocks = self.memory.ingest(blocks);
                }
                ScanOutcome::Unchanged { bounds } => {
                    self.bounds = bounds;
                    self.last_error = None;
                }
                ScanOutcome::Gone => *lost = true,
                ScanOutcome::Failed(message) => self.last_error = Some(message),
            }
        }
        if !self.scanner.is_busy() && self.last_scan.elapsed() >= self.interval {
            self.last_scan = Instant::now();
            self.scanner.submit(ScanJob {
                target: self.target.clone(),
                helper: self.helper.clone(),
                fake_image: self.fake_image.clone(),
            });
        }
        self.process_events();
        self.submit_translations();
    }

    fn app(&self) -> Option<String> {
        self.target.app().map(str::to_owned)
    }

    /// 后端的结果：流式中途的前面几条先存下，最后一条可能还没写完，等 Finished。
    fn process_events(&mut self) {
        for event in self.service.poll() {
            let id = event.id();
            let Some(keys) = self.in_flight.get(&id).cloned() else {
                continue;
            };
            match event {
                CoachEvent::Partial {
                    output: CoachOutput::Screen(screened),
                    ..
                } => {
                    let complete = screened.items.len().saturating_sub(1);
                    for item in screened.items.iter().take(complete) {
                        self.store_item(&keys, item.i, &item.translation, &item.note);
                    }
                }
                CoachEvent::Finished {
                    output: CoachOutput::Screen(screened),
                    ..
                } => {
                    for item in &screened.items {
                        self.store_item(&keys, item.i, &item.translation, &item.note);
                    }
                    self.memory.clear_pending(&keys);
                    self.in_flight.remove(&id);
                }
                CoachEvent::Failed { message, .. } => {
                    self.memory.clear_pending(&keys);
                    self.in_flight.remove(&id);
                    self.last_error = Some(message);
                    self.retry_after = Instant::now() + RETRY_DELAY;
                }
                _ => {}
            }
        }
    }

    fn store_item(&mut self, keys: &[String], index: usize, translation: &str, note: &str) {
        let Some(key) = index.checked_sub(1).and_then(|i| keys.get(i)) else {
            return;
        };
        if translation.trim().is_empty() {
            return;
        }
        if self.memory.analysis(key).is_none() {
            self.translated += 1;
        }
        self.memory.store(
            key,
            Analysis {
                translation: translation.trim().to_owned(),
                note: note.trim().to_owned(),
            },
        );
        for block in self.blocks.iter_mut().filter(|block| block.key == *key) {
            block.analysis = self.memory.analysis(key).cloned();
            block.pending = false;
        }
    }

    /// 屏幕上有新的英文块就批量交给后端；鼠标正指着的那块没翻译就插队。
    fn submit_translations(&mut self) {
        if self.in_flight.len() >= MAX_IN_FLIGHT || Instant::now() < self.retry_after {
            return;
        }
        let app = self.app();
        let gate = &self.gate;
        let mut wanted = self.memory.to_analyze(&self.blocks, BATCH, |text| {
            gate.allow_screen_block(text, app.as_deref())
        });
        // 鼠标指着的块没翻译：放最前面，哪怕已经有一批在飞
        if let Some(hovered) = self.hovered_block()
            && hovered.analysis.is_none()
            && !hovered.pending
            && self.gate.allow_screen_block(&hovered.text, app.as_deref())
            && !wanted.iter().any(|(key, _)| *key == hovered.key)
        {
            wanted.insert(0, (hovered.key.clone(), hovered.text.clone()));
            wanted.truncate(BATCH);
        }
        if wanted.is_empty() || (self.in_flight.len() == 1 && !self.hover_needs_priority()) {
            return;
        }
        let keys: Vec<String> = wanted.iter().map(|(key, _)| key.clone()).collect();
        let numbered: String = wanted
            .iter()
            .enumerate()
            .map(|(index, (_, text))| format!("{}. {text}", index + 1))
            .collect::<Vec<_>>()
            .join("\n");
        let first = self
            .blocks
            .iter()
            .position(|block| block.key == keys[0])
            .unwrap_or(0);
        let before = self.blocks[first.saturating_sub(CONTEXT_BLOCKS)..first]
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        self.next_id += 1;
        let id = self.next_id;
        let request = CoachRequest {
            id,
            mode: Mode::Screen,
            text: numbered,
            context: CoachContext {
                app,
                before,
                peer_message: None,
            },
        };
        if self.service.submit(request).is_ok() {
            self.memory.mark_pending(&keys);
            self.in_flight.insert(id, keys);
        }
    }

    /// 已经有一个批量请求在飞时，只有鼠标指着的块没翻译才值得再发一个插队的。
    fn hover_needs_priority(&self) -> bool {
        self.hovered_block()
            .is_some_and(|block| block.analysis.is_none() && !block.pending)
    }

    fn hovered_block(&self) -> Option<&ScreenBlock> {
        let key = self.hover_key.as_ref()?;
        self.blocks.iter().find(|block| block.key == *key)
    }

    /// 鼠标停在哪一块、停了多久；够久了就弹出那一块的悬浮卡，离开了就收起。
    fn update_card(&mut self, card: &mut CoachPanel) {
        let (x, y) = mouse_cg();
        let hovered = block_at(&self.blocks, x, y, HOVER_SLACK).cloned();
        let Some(block) = hovered else {
            self.hover_key = None;
            self.card_state = None;
            card.hide();
            return;
        };
        if self.hover_key.as_deref() != Some(block.key.as_str()) {
            self.hover_key = Some(block.key.clone());
            self.hover_since = Instant::now();
            self.card_state = None;
            card.hide();
            return;
        }
        if self.hover_since.elapsed() < HOVER_DELAY {
            return;
        }
        let state = (block.key.clone(), block.analysis.is_some(), block.pending);
        if self.card_state.as_ref() == Some(&state) {
            return;
        }
        let eligible = self
            .gate
            .allow_screen_block(&block.text, self.app().as_deref());
        let doc = match &block.analysis {
            Some(analysis) => Doc::screen_card(&analysis.translation, &analysis.note),
            None if !eligible => {
                self.card_state = Some(state);
                card.hide();
                return;
            }
            None if block.pending => Doc::screen_status("翻译中…"),
            None => Doc::screen_status(self.last_error.as_deref().unwrap_or("等待翻译…")),
        };
        self.card_state = Some(state);
        let primary = primary_height();
        let rect = block.rect;
        let anchor = NSRect::new(
            NSPoint::new(
                f64::from(rect.x),
                geometry::cg_to_cocoa_y(rect.y, rect.height, primary),
            ),
            NSSize::new(f64::from(rect.width), f64::from(rect.height)),
        );
        card.show(
            &PanelContent {
                doc,
                buttons: Vec::new(),
                footer: String::new(),
            },
            anchor,
        );
    }

    /// 状态条：目标窗口右上角一个小标记，告诉用户屏幕正在被读（隐私上必须一直看得见）。
    fn update_pill(&mut self, pill: &mut CoachPanel) {
        if self.last_pill.elapsed() < Duration::from_millis(700) {
            return;
        }
        self.last_pill = Instant::now();
        let text = match &self.last_error {
            _ if self.paused => "● 屏幕阅读已暂停（正在输入密码）".to_owned(),
            Some(error) => format!("● 屏幕阅读 · {error}"),
            None => format!(
                "● 屏幕阅读中 · {} · 已翻译 {} 条",
                self.target.label(),
                self.translated
            ),
        };
        let bounds = self.bounds;
        let top_left = NSPoint::new(
            f64::from(bounds.x + bounds.width) - PILL_WIDTH - 8.0,
            geometry::cg_to_cocoa_y(bounds.y, 0.0, primary_height()) - 8.0,
        );
        pill.show_at(
            &PanelContent {
                doc: Doc::screen_status(&text),
                buttons: Vec::new(),
                footer: String::new(),
            },
            top_left,
        );
    }
}

/// 主屏高度：AppKit 与 CG 坐标互换要用。
fn primary_height() -> f64 {
    MainThreadMarker::new()
        .and_then(|mtm| NSScreen::screens(mtm).firstObject())
        .map_or(0.0, |screen| screen.frame().size.height)
}

/// 鼠标位置，CG 坐标（原点在主屏左上）。
fn mouse_cg() -> (f32, f32) {
    let mouse = NSEvent::mouseLocation();
    geometry::cocoa_to_cg(mouse.x, mouse.y, primary_height())
}

/// 窗口所属应用的 bundle id。
fn bundle_of(pid: i32) -> Option<String> {
    objc2_app_kit::NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
        .and_then(|app| app.bundleIdentifier())
        .map(|id| id.to_string())
}
