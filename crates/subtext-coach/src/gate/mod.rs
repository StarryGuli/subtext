//! 外发闸门：决定一段文本能不能交给后端。所有触发源（复制、上屏）都先过这里。

pub mod language;
pub mod secrets;

use crate::Trigger;

/// 文本被拦下的原因，只用于日志与测试。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    Empty,
    TooShort,
    TooLong,
    NotTheRightLanguage,
    MachineText,
    LooksLikeSecret,
    Concealed,
    SkippedApp,
}

/// 闸门参数。
#[derive(Debug, Clone)]
pub struct Gate {
    /// 单次最多发多少字符。
    pub max_chars: usize,

    /// 不处理的应用（bundle id，小写比较）。
    pub skip_apps: Vec<String>,
}

/// 解码至少要这么多英文单词，免得每次复制一个单词都弹面板。
const MIN_DECODE_WORDS: usize = 3;

/// 组句至少要这么多汉字。
const MIN_COMPOSE_CJK: usize = 4;

/// 改稿至少要这么多英文单词。
const MIN_EDIT_WORDS: usize = 3;

impl Gate {
    pub fn new(max_chars: usize, skip_apps: Vec<String>) -> Self {
        Self {
            max_chars,
            skip_apps,
        }
    }

    /// 能发返回 `Ok(())`。`concealed` 是剪贴板上带的「隐蔽 / 瞬时」标记（密码管理器会打）。
    pub fn check(
        &self,
        trigger: Trigger,
        text: &str,
        app: Option<&str>,
        concealed: bool,
    ) -> Result<(), Skip> {
        if concealed {
            return Err(Skip::Concealed);
        }
        if let Some(app) = app
            && self
                .skip_apps
                .iter()
                .any(|skip| skip.eq_ignore_ascii_case(app))
        {
            return Err(Skip::SkippedApp);
        }
        let text = text.trim();
        if text.is_empty() {
            return Err(Skip::Empty);
        }
        if text.chars().count() > self.max_chars {
            return Err(Skip::TooLong);
        }
        if trigger.mode_for(text).is_none() {
            return Err(Skip::NotTheRightLanguage);
        }
        if language::looks_like_machine_text(text) {
            return Err(Skip::MachineText);
        }
        if secrets::contains_secret(text) {
            return Err(Skip::LooksLikeSecret);
        }
        let long_enough = match trigger {
            Trigger::ClipboardCopy => language::english_words(text) >= MIN_DECODE_WORDS,
            Trigger::ChineseCommitted => language::cjk_chars(text) >= MIN_COMPOSE_CJK,
            Trigger::EnglishDraft => language::english_words(text) >= MIN_EDIT_WORDS,
        };
        if long_enough {
            Ok(())
        } else {
            Err(Skip::TooShort)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate() -> Gate {
        Gate::new(200, vec!["com.1password.1password".to_owned()])
    }

    const PR: &str = "hey, no rush at all but did you get a chance to look at that PR?";

    #[test]
    fn ordinary_english_copy_passes() {
        assert_eq!(
            gate().check(Trigger::ClipboardCopy, PR, None, false),
            Ok(())
        );
    }

    #[test]
    fn concealed_and_skipped_apps_are_blocked_first() {
        assert_eq!(
            gate().check(Trigger::ClipboardCopy, PR, None, true),
            Err(Skip::Concealed)
        );
        assert_eq!(
            gate().check(
                Trigger::ClipboardCopy,
                PR,
                Some("COM.1PASSWORD.1PASSWORD"),
                false
            ),
            Err(Skip::SkippedApp)
        );
    }

    #[test]
    fn secrets_links_and_long_text_are_blocked() {
        let g = gate();
        assert_eq!(
            g.check(
                Trigger::ClipboardCopy,
                "my password is hunter2 please keep it",
                None,
                false
            ),
            Err(Skip::LooksLikeSecret)
        );
        assert_eq!(
            g.check(
                Trigger::ClipboardCopy,
                "https://example.com/a/very/long/path",
                None,
                false
            ),
            Err(Skip::MachineText)
        );
        assert_eq!(
            g.check(Trigger::ClipboardCopy, &"word ".repeat(100), None, false),
            Err(Skip::TooLong)
        );
    }

    #[test]
    fn short_text_and_wrong_language_are_skipped() {
        let g = gate();
        assert_eq!(
            g.check(Trigger::ClipboardCopy, "thanks a lot", None, false),
            Err(Skip::TooShort)
        );
        assert_eq!(
            g.check(Trigger::ClipboardCopy, "今天开会吗", None, false),
            Err(Skip::NotTheRightLanguage)
        );
        assert_eq!(
            g.check(Trigger::ChineseCommitted, "好的", None, false),
            Err(Skip::TooShort)
        );
        assert_eq!(
            g.check(Trigger::ChineseCommitted, "我这周实验做不完", None, false),
            Ok(())
        );
    }
}
