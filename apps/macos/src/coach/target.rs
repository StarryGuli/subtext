//! 面板按钮的 target：一个 `clicked:` 选择器，靠 tag 区分是哪个按钮。

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_foundation::{NSObject, NSObjectProtocol};

use super::action::CoachAction;
use crate::host;

define_class!(
    // SAFETY: NSObject 没有子类化要求；没有实现 Drop。
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    pub struct CoachTarget;

    impl CoachTarget {
        #[unsafe(method(clicked:))]
        fn clicked(&self, sender: Option<&AnyObject>) {
            let Some(sender) = sender else { return };
            // SAFETY: 只有 NSButton 把 clicked: 当 action，它是 NSControl
            let tag: isize = unsafe { msg_send![sender, tag] };
            if let Some(action) = CoachAction::from_tag(tag) {
                host::coach_perform(action);
            }
        }
    }

    unsafe impl NSObjectProtocol for CoachTarget {}
);

impl CoachTarget {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}
