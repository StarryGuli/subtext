//! 一次性识别：用力按压时读一次鼠标所在的窗口，找出鼠标下面那一块文字。不持续读，也不翻译，结果交给教练解析。

use std::path::PathBuf;
use std::time::{Duration, Instant};

use subtext_coach::screen::{Rect, group_blocks};

use super::scan::{ScanJob, ScanOutcome, Scanner};
use super::target::Target;

/// 鼠标离块边缘这么近（点）也算指着它。
const SLACK: f32 = 6.0;

/// 等多久还没结果就放弃。
const TIMEOUT: Duration = Duration::from_secs(15);

/// 读不到窗口时，取鼠标周围这么大一块。
const FALLBACK_WIDTH: f32 = 900.0;
const FALLBACK_HEIGHT: f32 = 600.0;

pub(super) struct Peek {
    scanner: Scanner,

    mouse: (f32, f32),

    started: Instant,
}

impl Peek {
    pub(super) fn start(target: Target, helper: PathBuf, mouse: (f32, f32)) -> Self {
        let mut scanner = Scanner::start();
        scanner.submit(ScanJob {
            target,
            helper,
            fake_image: None,
        });
        Self {
            scanner,
            mouse,
            started: Instant::now(),
        }
    }

    /// 鼠标周围一块区域（窗口列表里没有鼠标下的窗口时用）。
    pub(super) fn region_around(x: f32, y: f32) -> Rect {
        Rect::new(
            (x - FALLBACK_WIDTH / 2.0).max(0.0),
            (y - FALLBACK_HEIGHT / 2.0).max(0.0),
            FALLBACK_WIDTH,
            FALLBACK_HEIGHT,
        )
    }

    /// 结果：`None` 还没好；`Some(Ok(Some(文字)))` 读到了鼠标下的那块；`Ok(None)` 鼠标下没有文字。
    pub(super) fn poll(&mut self) -> Option<Result<Option<String>, String>> {
        match self.scanner.poll() {
            Some(ScanOutcome::Lines { lines, .. }) => {
                let blocks = group_blocks(&lines);
                // 几块都包含这个点时取最小的（最具体的）
                let text = blocks
                    .iter()
                    .filter(|block| block.rect.contains(self.mouse.0, self.mouse.1, SLACK))
                    .min_by(|a, b| {
                        (a.rect.width * a.rect.height).total_cmp(&(b.rect.width * b.rect.height))
                    })
                    .map(|block| block.text.clone());
                Some(Ok(text))
            }
            Some(ScanOutcome::Unchanged { .. }) => Some(Ok(None)),
            Some(ScanOutcome::Gone) => Some(Err("鼠标下的窗口不见了".to_owned())),
            Some(ScanOutcome::Failed(message)) => Some(Err(message)),
            None if self.started.elapsed() > TIMEOUT => Some(Err("识别超时".to_owned())),
            None => None,
        }
    }
}
