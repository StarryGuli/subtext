//! 屏幕阅读的轮询定时器：阅读期间每 0.12 秒一次，鼠标停在哪句要尽快反应；不阅读时不跑。

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_foundation::{NSObject, NSObjectProtocol, NSTimer};

const POLL_INTERVAL: f64 = 0.12;

pub struct ScreenMonitor {
    timer: Option<Retained<NSTimer>>,

    mtm: MainThreadMarker,
}

impl ScreenMonitor {
    pub fn new(mtm: MainThreadMarker) -> Self {
        Self { timer: None, mtm }
    }

    pub fn start(&mut self) {
        if self.timer.is_some() {
            return;
        }
        let target = ScreenTicker::new(self.mtm);
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
    struct ScreenTicker;

    impl ScreenTicker {
        #[unsafe(method(tick:))]
        fn tick(&self, _timer: Option<&AnyObject>) {
            crate::host::screen_tick();
        }
    }

    unsafe impl NSObjectProtocol for ScreenTicker {}
);

impl ScreenTicker {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}
