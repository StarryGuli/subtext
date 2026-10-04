//! 屏幕阅读的宿主侧胶水：定时回调、菜单与快捷键的入口。

use objc2_foundation::NSRect;

use super::with;

/// 屏幕阅读的定时器每 0.12 秒来一次；有话要告诉用户（开始、停止、出错）就在候选窗的提示条里显示。
pub fn screen_tick() {
    let notice = with(|h| {
        h.screen.tick();
        // 用力按压读到的文字：交给教练现在就解析
        let forced = h
            .screen
            .take_peek_text()
            .and_then(|text| h.coach.submit_forced(text));
        forced.or_else(|| h.screen.take_notice())
    })
    .flatten();
    if let Some(text) = notice {
        with(|h| h.show_notice(&text, NSRect::ZERO));
    }
}

/// 选框层选完了（或取消）：只记结果，由屏幕阅读的轮询去关选框层、开始读。
pub fn screen_region_done(region: Option<subtext_coach::screen::Rect>) {
    with(|h| h.screen.region_done(region));
}

/// 快捷键：正在读就停；没在读就读鼠标所在的窗口。
pub fn screen_toggle() {
    let notice = with(|h| {
        if h.screen.is_reading() {
            h.screen.stop();
        } else if let Err(message) = h.screen.start_window_under_mouse() {
            return Some(message);
        }
        h.screen.take_notice()
    })
    .flatten();
    if let Some(text) = notice {
        with(|h| h.show_notice(&text, NSRect::ZERO));
    }
}
