//! 屏幕阅读的入口：选目标（鼠标下的窗口 / 框选区域）、开始与停止、每 0.12 秒推进一次。

use std::time::{Duration, Instant};

use objc2::MainThreadMarker;
use subtext_coach::{CoachConfig, SharedCache};

use super::active::{Active, PILL_WIDTH};
use super::desktop::{bundle_of, mouse_cg};
use super::monitor::ScreenMonitor;
use super::peek::Peek;
use super::region::RegionPicker;
use super::target::Target;
use super::{capture, geometry, ocr, windows};
use crate::coach::CoachPanel;

const CARD_WIDTH: f64 = 340.0;

pub struct ScreenReader {
    config: CoachConfig,

    cache: SharedCache,

    monitor: ScreenMonitor,

    card: CoachPanel,

    pill: CoachPanel,

    active: Option<Active>,

    /// 教练被关掉时屏幕阅读只暂停：不截屏不发送，识别过的内容留着，重新打开教练就接着读。
    suspended: bool,

    /// 用力按压触发的一次性识别（不是持续阅读）。
    peek: Option<Peek>,

    /// 一次性识别读到的鼠标下那块文字，等宿主取走交给教练。
    peek_text: Option<String>,

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
            cache: SharedCache::in_memory(),
            monitor: ScreenMonitor::new(mtm),
            card,
            pill,
            active: None,
            suspended: false,
            peek: None,
            peek_text: None,
            pick_at: None,
            notice: None,
            picker: None,
            region_result: None,
            mtm,
        }
    }

    pub fn apply(&mut self, config: &CoachConfig, cache: SharedCache) {
        let changed = self.config != *config;
        self.config = config.clone();
        self.cache = cache;
        if !config.enabled {
            if self.active.is_none() {
                self.stop();
            } else if !self.suspended {
                self.suspended = true;
                self.card.hide();
                self.pill.hide();
                self.notice = Some("教练已关闭，屏幕阅读先暂停；重新打开教练就接着读".to_owned());
            }
            return;
        }
        let resumed = self.suspended;
        self.suspended = false;
        if (resumed || changed)
            && let Some(active) = self.active.as_mut()
        {
            active.rebuild_service(config, self.cache.clone());
            if resumed {
                self.notice = Some("屏幕阅读继续".to_owned());
            }
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

    /// 截屏权限与 OCR 程序都就绪才返回帮手程序路径；否则是要告诉用户的话。
    fn ready(&self) -> Result<std::path::PathBuf, String> {
        if !self.config.enabled {
            return Err("请先在偏好设置里启用双语教练".to_owned());
        }
        if !capture::has_permission() {
            capture::request_permission();
            tracing::warn!("屏幕录制权限检查未通过（CGPreflightScreenCaptureAccess=false）");
            return Err(
                "屏幕阅读需要「屏幕录制」权限：请在 系统设置 → 隐私与安全性 → 屏幕录制 里允许 Subtext，授权后重新开始。已经打开还提示：重新安装后旧授权会失效，把开关关掉再打开，或在终端运行 tccutil reset ScreenCapture app.subtext.inputmethod 后重新授权，再注销登录一次"
                    .to_owned(),
            );
        }
        ocr::helper_path().ok_or_else(|| "找不到 OCR 程序 subtext-ocr，请重新安装".to_owned())
    }

    /// 开始阅读 `target`。教练没开、没权限、没有 OCR 程序都会返回要告诉用户的话。
    pub fn start(&mut self, target: Target) -> Result<(), String> {
        let helper = self.ready()?;
        self.stop();
        let label = target.label();
        self.active = Some(Active::new(
            target,
            helper,
            &self.config,
            self.cache.clone(),
        ));
        self.monitor.start();
        self.notice = Some(format!("开始屏幕阅读：{label}。鼠标停在英文上就会显示译文"));
        Ok(())
    }

    /// 停止：丢掉识别过的屏幕文字，收起悬浮卡与状态条。
    pub fn stop(&mut self) {
        let was_reading = self.active.take().is_some();
        self.suspended = false;
        self.pick_at = None;
        self.monitor.stop();
        self.card.hide();
        self.pill.hide();
        if was_reading {
            self.notice = Some("已停止屏幕阅读".to_owned());
        }
    }

    /// 用力按压：读鼠标所在的窗口（读不到窗口就读鼠标周围一块），找出鼠标下那一块文字。
    pub fn begin_peek(&mut self) {
        if self.peek.is_some() {
            return;
        }
        let helper = match self.ready() {
            Ok(helper) => helper,
            Err(message) => {
                self.notice = Some(message);
                return;
            }
        };
        let (x, y) = mouse_cg();
        let own_pid = std::process::id() as i32;
        let target = match geometry::pick_window(&windows::list(), x, y, own_pid) {
            Some(window) => Target::Window {
                id: window.id,
                owner: window.owner.clone(),
                bundle: bundle_of(window.pid),
                bounds: window.bounds,
            },
            None => Target::Region {
                bounds: Peek::region_around(x, y),
            },
        };
        self.peek = Some(Peek::start(target, helper, (x, y)));
        self.monitor.start();
    }

    /// 取走一次性识别读到的文字。
    pub fn take_peek_text(&mut self) -> Option<String> {
        self.peek_text.take()
    }

    fn poll_peek(&mut self) {
        let Some(peek) = self.peek.as_mut() else {
            return;
        };
        let Some(result) = peek.poll() else {
            return;
        };
        self.peek = None;
        match result {
            Ok(Some(text)) => self.peek_text = Some(text),
            Ok(None) => self.notice = Some("鼠标下面没有识别到文字".to_owned()),
            Err(message) => self.notice = Some(message),
        }
        if self.active.is_none() && self.pick_at.is_none() && self.picker.is_none() {
            self.monitor.stop();
        }
    }

    /// 每 0.12 秒一次。
    pub fn tick(&mut self) {
        self.poll_peek();
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
        if self.suspended {
            return;
        }
        let Some(active) = self.active.as_mut() else {
            return;
        };
        // 系统在输入密码（Secure Input）：暂停读取，什么都不截、不发，悬浮卡也收起
        if crate::imk::secure_input::enabled() {
            active.set_paused(true);
            self.card.hide();
            active.update_pill(&mut self.pill);
            return;
        }
        active.set_paused(false);
        if active.drive() {
            self.notice = Some("阅读的窗口很久没出现了，已停止屏幕阅读".to_owned());
            self.stop();
            return;
        }
        active.update_card(&mut self.card);
        active.update_pill(&mut self.pill);
    }
}
