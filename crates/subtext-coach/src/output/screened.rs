//! 屏幕阅读的结果：对一批屏幕消息，逐条给译文与一句提示。

use serde::{Deserialize, Serialize};

/// 一条消息的结果；`i` 是请求里的编号（从 1 数）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScreenItem {
    pub i: usize,

    /// 译文：英文译成中文。OCR 识别错的地方模型会按上下文还原后再译。
    pub translation: String,

    /// 一句语气 / 俚语 / 潜台词提示，没有值得说的就空。
    pub note: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Screened {
    pub items: Vec<ScreenItem>,
}

impl Screened {
    pub fn is_empty(&self) -> bool {
        self.items
            .iter()
            .all(|item| item.translation.trim().is_empty())
    }
}
