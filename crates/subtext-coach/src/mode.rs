//! 教练的三种模式，以及由「发生了什么」决定用哪一种。

use serde::{Deserialize, Serialize};

use crate::gate::language;

/// 对应 `english-message-coach` 的 A / B / C。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// 读懂收到的英文。
    Decode,

    /// 把中文组成要发出去的英文。
    Compose,

    /// 修改自己写的英文草稿。
    Edit,
}

/// 触发教练的事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// 剪贴板里出现了新复制的文本。
    ClipboardCopy,

    /// 刚上屏了一句中文。
    ChineseCommitted,

    /// 在英文输入状态下写完了一句。
    EnglishDraft,
}

impl Trigger {
    /// 按文本的实际语言取模式：复制来的中文不解码，上屏的英文不翻译，返回 `None` 表示什么都不做。
    pub fn mode_for(self, text: &str) -> Option<Mode> {
        let cjk = language::cjk_chars(text);
        let english = language::is_mostly_english(text);
        match self {
            Self::ClipboardCopy if english => Some(Mode::Decode),
            Self::ChineseCommitted if cjk > 0 && !english => Some(Mode::Compose),
            Self::EnglishDraft if english => Some(Mode::Edit),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copied_english_is_decoded_and_copied_chinese_is_ignored() {
        let text = "hey, no rush at all but did you get a chance to look at that PR?";
        assert_eq!(Trigger::ClipboardCopy.mode_for(text), Some(Mode::Decode));
        assert_eq!(Trigger::ClipboardCopy.mode_for("你好，今天开会吗"), None);
    }

    #[test]
    fn committed_chinese_is_composed_and_english_is_not() {
        assert_eq!(
            Trigger::ChineseCommitted.mode_for("我这周实验做不完"),
            Some(Mode::Compose)
        );
        assert_eq!(
            Trigger::ChineseCommitted.mode_for("see you tomorrow at the lab"),
            None
        );
    }

    #[test]
    fn english_draft_is_edited() {
        assert_eq!(
            Trigger::EnglishDraft.mode_for("i cant make it to the meeting tmrw"),
            Some(Mode::Edit)
        );
    }
}
