//! 教练的轮询定时器：输入法激活期间每 0.3 秒看一次剪贴板、上屏停顿与后台事件。

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_foundation::{NSObject, NSObjectProtocol, NSTimer};

/// 轮询间隔。
const POLL_INTERVAL: f64 = 0.3;

pub struct CoachMonitor {
    timer: Option<Retained<NSTimer>>,

    mtm: MainThreadMarker,
}

impl CoachMonitor {
    pub fn new(mtm: MainThreadMarker) -> Self {
        Self { timer: None, mtm }
    }

    pub fn start(&mut self) {
        if self.timer.is_some() {
            return;
        }
        let target = CoachTicker::new(self.mtm);
        let timer = unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                POLL_INTERVAL,
                &target,
                sel!(tick:),
                None,
                true,
            )
        };
        self.timer = Some(timer);
    }

    pub fn stop(&mut self) {
        if let Some(timer) = self.timer.take() {
            timer.invalidate();
        }
    }
}

define_class!(
    // SAFETY: NSObject 没有子类化要求；没有实现 Drop。
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    struct CoachTicker;

    impl CoachTicker {
        #[unsafe(method(tick:))]
        fn tick(&self, _timer: Option<&AnyObject>) {
            crate::host::coach_tick();
        }
    }

    unsafe impl NSObjectProtocol for CoachTicker {}
);

impl CoachTicker {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}
