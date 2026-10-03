//! 把应用里选中的文字交给双语教练：英文当草稿改稿，中文给出英文表达，别人发来的英文解码。

use super::*;

/// 选中文字最多交给教练多少个字符；再长闸门也会拦，这里先拦住少读一次应用。
const MAX_COACH_CHARS: usize = 1500;

impl SubtextInputController {
    /// 教练快捷键：教练关着、密码框、没有选区都不动（键交回应用）。
    pub(super) fn coach_selection(&self, client: TextClient<'_>) -> bool {
        if !host::with(|h| h.coach.is_enabled()).unwrap_or(false) {
            tracing::info!("双语教练没开，快捷键不生效");
            return false;
        }
        if secure_input::enabled() {
            tracing::debug!("Secure Input 中，不交给教练");
            return false;
        }
        // 光标位置与选区都要等应用回话，先在借用之外取好
        let anchor = client.caret_rect();
        let Some((text, range)) = client.selected_text(MAX_COACH_CHARS) else {
            host::with(|h| {
                h.show_notice(
                    "没有选中的文字，或这个应用不支持读取选区（最多 1500 字）",
                    anchor,
                )
            });
            return true;
        };
        let started = host::with(|h| {
            let app = h.engine.application().map(str::to_owned);
            h.coach
                .submit_selection(&text, range, client.object(), app, anchor)
        })
        .unwrap_or(false);
        if !started {
            host::with(|h| {
                h.show_notice("这段文字不适合交给教练（太短、疑似密钥或语言不对）", anchor)
            });
        }
        true
    }
}
