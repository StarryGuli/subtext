//! 阅读中的一切：后台扫描、消息块、翻译与预解码的在飞请求、悬浮卡状态。停下来就整个丢掉（含识别过的屏幕文字）。

mod drive;
mod hover;
mod submit;

pub(in crate::screen) use hover::PILL_WIDTH;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use subtext_coach::gate::Gate;
use subtext_coach::screen::{Rect, ScreenBlock, ScreenMemory};
use subtext_coach::{CoachConfig, CoachOutput, CoachService, SharedCache};

use super::scan::Scanner;
use super::target::Target;

/// 一次最多翻译几块；多了留最新的，旧的等下一批。
const BATCH: usize = 5;

/// 翻译失败后隔多久再试。
const RETRY_DELAY: Duration = Duration::from_secs(8);

/// 给模型看的上文最多几块。
const CONTEXT_BLOCKS: usize = 6;

/// 同时最多几个后台翻译请求在飞：一个后台批量，一个鼠标指着的那块插队。
const MAX_IN_FLIGHT: usize = 2;

/// 目标窗口连续这么久读不到（换了桌面空间、最小化）才算真的没了；期间只是暂停。
const GONE_LIMIT: Duration = Duration::from_secs(600);

/// 预解码结果最多留几条。
const DECODED_LIMIT: usize = 80;

/// 同一块翻译失败几次就放弃。
const MAX_FAILURES: u8 = 2;

pub(in crate::screen) struct Active {
    target: Target,

    helper: PathBuf,

    scanner: Scanner,

    /// 屏幕阅读自己的后端线程（排队模式，一个请求都不丢），缓存与输入侧共用。
    service: CoachService,

    gate: Gate,

    interval: Duration,

    prewarm: usize,

    memory: ScreenMemory,

    blocks: Vec<ScreenBlock>,

    /// 在飞的翻译请求：编号 → 它负责的块标识。
    in_flight: HashMap<u64, Vec<String>>,

    /// 在飞的预解码请求：编号 → 块标识。
    warming: HashMap<u64, String>,

    /// 已经预解码好的完整解读（块标识 → 输出）。
    decoded: HashMap<String, CoachOutput>,

    /// 预解码失败过的块，不再重试。
    warm_failed: HashSet<String>,

    /// 翻译失败过几次的块：连续失败两次就不再重试，免得同一批文字每隔几秒重发一遍。
    failures: HashMap<String, u8>,

    next_id: u64,

    last_scan: Instant,

    last_error: Option<String>,

    retry_after: Instant,

    /// 读不到目标的起点；读到了就清空。
    gone_since: Option<Instant>,

    hover_key: Option<String>,

    hover_since: Instant,

    /// 鼠标离开块的起点：给一小段宽限，让鼠标能移到悬浮卡上。
    leave_since: Option<Instant>,

    /// 悬浮卡正显示的内容：(块标识, 有无译文, 是否在翻译, 是否完整解读)，变了才重画。
    card_state: Option<(String, bool, bool, bool)>,

    last_pill: Instant,

    /// 已经拿到译文的块数，状态条上显示。
    translated: usize,

    /// 系统正在输入密码：暂停中。
    paused: bool,

    /// 目标此刻在屏幕上的位置：窗口会被拖动，状态条跟着它走。
    bounds: Rect,

    /// 自测用：不真的截屏。
    fake_image: Option<PathBuf>,
}

impl Active {
    pub(in crate::screen) fn new(
        target: Target,
        helper: PathBuf,
        config: &CoachConfig,
        cache: SharedCache,
    ) -> Self {
        let bounds = target.bounds();
        Self {
            target,
            helper,
            scanner: Scanner::start(),
            service: CoachService::start_queued(config, cache),
            gate: Gate::new(config.max_chars, config.skip_apps.clone()),
            interval: Duration::from_millis(config.screen_interval_ms.max(500)),
            prewarm: config.screen_prewarm,
            memory: ScreenMemory::default(),
            blocks: Vec::new(),
            in_flight: HashMap::new(),
            warming: HashMap::new(),
            decoded: HashMap::new(),
            warm_failed: HashSet::new(),
            failures: HashMap::new(),
            next_id: 0,
            last_scan: Instant::now() - Duration::from_secs(60),
            last_error: None,
            retry_after: Instant::now(),
            gone_since: None,
            hover_key: None,
            hover_since: Instant::now(),
            leave_since: None,
            card_state: None,
            last_pill: Instant::now() - Duration::from_secs(60),
            translated: 0,
            paused: false,
            bounds,
            fake_image: None,
        }
    }

    /// 换了后端设置或教练重新打开：换一个新的后端线程，在飞的请求作废（会重新翻译），已经翻好的译文留着。
    pub(in crate::screen) fn rebuild_service(&mut self, config: &CoachConfig, cache: SharedCache) {
        let pending: Vec<String> = self.in_flight.values().flatten().cloned().collect();
        self.memory.clear_pending(&pending);
        self.in_flight.clear();
        self.warming.clear();
        self.service = CoachService::start_queued(config, cache);
        self.gate = Gate::new(config.max_chars, config.skip_apps.clone());
        self.interval = Duration::from_millis(config.screen_interval_ms.max(500));
        self.prewarm = config.screen_prewarm;
        self.retry_after = Instant::now();
    }

    /// 自测用：当前识别出的块与它们的译文。
    pub(in crate::screen) fn snapshot(&self) -> &[ScreenBlock] {
        &self.blocks
    }

    pub(in crate::screen) fn decoded_count(&self) -> usize {
        self.decoded.len()
    }

    pub(in crate::screen) fn set_fake_image(&mut self, image: PathBuf) {
        self.fake_image = Some(image);
    }

    pub(in crate::screen) fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    fn app(&self) -> Option<String> {
        self.target.app().map(str::to_owned)
    }
}
