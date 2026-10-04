//! 触控板用力按压（Force Touch）的检测：全局监听压力事件，压到第二档（stage 2）记一次。
//!
//! 全局监听只能「看」，不能拦：系统自己的「查询」可能同时弹出，要在系统设置 → 触控板 → 查询与数据检测器里改成别的手势。

use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSEvent, NSEventMask};

/// 压到第二档、还没被取走的那一次。
static PRESSED: AtomicBool = AtomicBool::new(false);

/// 上一次事件的档位，用来只认「从浅压到深压」的那一下，不认一直压着时反复来的事件。
static LAST_STAGE: AtomicIsize = AtomicIsize::new(0);

/// 压到第二档的档位值。
const DEEP_STAGE: isize = 2;

pub struct PressureWatch {
    monitor: Option<Retained<AnyObject>>,
}

impl PressureWatch {
    pub fn new() -> Self {
        Self { monitor: None }
    }

    pub fn start(&mut self) {
        if self.monitor.is_some() {
            return;
        }
        let handler = RcBlock::new(|event: NonNull<NSEvent>| {
            // SAFETY: AppKit 传进来的事件在回调期间有效
            let stage = unsafe { event.as_ref() }.stage();
            let previous = LAST_STAGE.swap(stage, Ordering::Relaxed);
            if stage >= DEEP_STAGE && previous < DEEP_STAGE {
                PRESSED.store(true, Ordering::Relaxed);
            }
        });
        self.monitor =
            NSEvent::addGlobalMonitorForEventsMatchingMask_handler(NSEventMask::Pressure, &handler);
        if self.monitor.is_none() {
            tracing::warn!("没能注册触控板压力监听");
        }
    }

    pub fn stop(&mut self) {
        if let Some(monitor) = self.monitor.take() {
            // SAFETY: monitor 是 addGlobalMonitor 返回的对象
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
        PRESSED.store(false, Ordering::Relaxed);
    }

    /// 取走「压了一下」的标记。
    pub fn take_press(&self) -> bool {
        PRESSED.swap(false, Ordering::Relaxed)
    }
}
