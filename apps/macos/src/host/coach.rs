//! 教练的宿主侧胶水：凡是碰应用客户端的步骤，都放在借用 Host 之外做（见 [`with`] 的重入说明）。

use objc2_foundation::NSRange;

use super::with;
use crate::coach::{CoachAction, ComposeProbe};
use crate::imk::TextClient;

/// 轮询定时器每 0.3 秒来一次。
pub fn coach_tick() {
    if let Some(message) = with(|h| h.coach.take_test_result()).flatten() {
        with(|h| h.preferences.set_status(&message));
    }
    let Some(probe) = with(|h| h.coach.tick()).flatten() else {
        return;
    };
    let range = locate(&probe);
    with(|h| h.coach.submit_compose(probe, range));
}

/// 在应用里确认刚上屏的那句中文还在光标前面，返回它的范围；确认不了就只能复制。
fn locate(probe: &ComposeProbe) -> Option<NSRange> {
    let client = TextClient::new(&probe.client);
    let selected = client.selected_range()?;
    let length = probe.text.encode_utf16().count();
    let range = NSRange::new(selected.location.checked_sub(length)?, length);
    (client.text_in_range(range)? == probe.text).then_some(range)
}

/// 面板上的按钮、快捷键都走这里。
pub fn coach_perform(action: CoachAction) {
    match action {
        CoachAction::Close => {
            with(|h| h.coach.dismiss());
        }
        CoachAction::Reveal => {
            with(|h| h.coach.reveal());
        }
        CoachAction::Copy(_) | CoachAction::CopyEdited => {
            with(|h| h.coach.copy(action));
        }
        CoachAction::Replace(_) | CoachAction::ReplaceEdited => {
            let Some(plan) = with(|h| h.coach.replace_plan(action)).flatten() else {
                // 没有可替换的位置：退而求其次复制
                if let CoachAction::Replace(index) = action {
                    with(|h| h.coach.copy(CoachAction::Copy(index)));
                }
                return;
            };
            let outcome = plan.execute();
            with(|h| h.coach.after_replace(&plan, outcome));
        }
    }
}

/// ⌥ + 数字：面板上有组句选项时，用对应那个替换刚输入的中文。返回 `true` 表示按键已被用掉。
pub fn coach_digit(digit: usize) -> bool {
    let Some(action) = with(|h| h.coach.digit_action(digit)).flatten() else {
        return false;
    };
    coach_perform(action);
    true
}

/// Esc：面板在显示时收起它。返回 `true` 表示按键已被用掉。
pub fn coach_escape() -> bool {
    with(|h| {
        let showing = h.coach.is_showing();
        if showing {
            h.coach.dismiss();
        }
        showing
    })
    .unwrap_or(false)
}

/// 刚往应用里上屏了一段文字，教练据此攒中文句子。
pub fn coach_note_commit(text: &str, client: TextClient<'_>) {
    with(|h| {
        let app = h.engine.application().map(str::to_owned);
        let anchor = h.anchor;
        h.coach.note_commit(text, client.object(), app, anchor);
    });
}
