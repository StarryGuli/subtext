//! 后台扫描线程：截屏 → 画面没变就跳过 → OCR → 换算成屏幕坐标。全部在这个线程里，不卡输入。
//!
//! 截图含屏幕上的任何内容，所以识别完立刻删掉，不留在磁盘上。

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

use subtext_coach::screen::{OcrLine, Rect};

use super::target::Target;
use super::{capture, geometry, ocr, windows};

/// 交给扫描线程的一次任务。
pub struct ScanJob {
    pub target: Target,

    /// OCR 帮手程序。
    pub helper: PathBuf,

    /// 自测用：不真的截屏，把这张图当作截屏结果。
    pub fake_image: Option<PathBuf>,
}

/// 一次扫描的结果。
#[derive(Debug)]
pub enum ScanOutcome {
    /// 识别到了这些行（屏幕点坐标），`bounds` 是此刻目标所在的屏幕区域。
    Lines {
        lines: Vec<OcrLine>,
        bounds: Rect,
    },

    /// 画面和上次一模一样，没有重新识别；位置也可能变了，所以带回 `bounds`。
    Unchanged {
        bounds: Rect,
    },

    /// 目标窗口没了（关了、最小化了）。
    Gone,

    Failed(String),
}

pub struct Scanner {
    jobs: Sender<ScanJob>,

    results: Receiver<ScanOutcome>,

    /// 有任务在跑：一次只跑一个，没回来之前不发下一个。
    busy: bool,
}

impl Scanner {
    pub fn start() -> Self {
        let (job_sender, job_receiver) = channel::<ScanJob>();
        let (result_sender, result_receiver) = channel();
        std::thread::Builder::new()
            .name("subtext-screen-scan".to_owned())
            .spawn(move || run(&job_receiver, &result_sender))
            .expect("spawn screen scan thread");
        Self {
            jobs: job_sender,
            results: result_receiver,
            busy: false,
        }
    }

    pub fn is_busy(&self) -> bool {
        self.busy
    }

    pub fn submit(&mut self, job: ScanJob) {
        if self.jobs.send(job).is_ok() {
            self.busy = true;
        }
    }

    pub fn poll(&mut self) -> Option<ScanOutcome> {
        let outcome = self.results.try_recv().ok()?;
        self.busy = false;
        Some(outcome)
    }
}

fn run(jobs: &Receiver<ScanJob>, results: &Sender<ScanOutcome>) {
    let image = std::env::temp_dir().join(format!("subtext-screen-{}.png", std::process::id()));
    let mut last_hash: Option<u64> = None;
    while let Ok(job) = jobs.recv() {
        let outcome = scan_once(&job, &image, &mut last_hash);
        let _ = std::fs::remove_file(&image);
        if results.send(outcome).is_err() {
            break;
        }
    }
}

fn scan_once(job: &ScanJob, image: &std::path::Path, last_hash: &mut Option<u64>) -> ScanOutcome {
    let bounds = match &job.target {
        Target::Window { id, .. } => match windows::bounds_of(*id) {
            Some(bounds) => bounds,
            None => return ScanOutcome::Gone,
        },
        Target::Region { bounds } => *bounds,
    };
    if let Some(fake) = &job.fake_image {
        if let Err(error) = std::fs::copy(fake, image) {
            return ScanOutcome::Failed(format!("读不了自测图片：{error}"));
        }
    } else if let Err(error) = capture::capture(&job.target, bounds, image) {
        return ScanOutcome::Failed(error);
    }
    let hash = std::fs::read(image).ok().map(|bytes| {
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish()
    });
    if hash.is_some() && hash == *last_hash {
        return ScanOutcome::Unchanged { bounds };
    }
    *last_hash = hash;
    match ocr::recognize(&job.helper, image) {
        Ok(lines) => ScanOutcome::Lines {
            lines: geometry::to_screen_lines(&lines, bounds),
            bounds,
        },
        Err(error) => ScanOutcome::Failed(error),
    }
}
