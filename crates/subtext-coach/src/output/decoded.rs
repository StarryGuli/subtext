//! 解码结果：情境、教学点、语气与潜台词、折叠的译文、一个引导问题。

use serde::{Deserialize, Serialize};

/// 一个值得学的点：俚语、缩写、套话、文化梗。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DecodePoint {
    /// 原文里的片段。
    pub phrase: String,

    /// `slang` / `abbr` / `hedge` / `culture` / `fixed`。
    pub kind: String,

    /// 字面意思；缩写则是展开形式。
    pub literal: String,

    /// 实际意思与说话人的用意。
    pub meaning: String,

    /// 中文对应；没有清爽对应时写明并说明为什么。
    pub zh: String,

    /// 一句用户自己能用的例子。
    pub usage: String,
}

/// 语气与潜台词。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tone {
    /// `casual` / `neutral` / `professional` / `passive-aggressive` / `warm` 等。
    pub register: String,

    /// 说了什么与想说什么的差别。
    pub subtext: String,

    /// 国内 / 美国对照，没有就留空。
    pub contrast: String,
}

/// 解码模式的完整输出。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Decoded {
    /// 一句话说清谁在说话、要什么，不翻译内容。
    pub situation: String,

    pub points: Vec<DecodePoint>,

    pub tone: Tone,

    /// 中文译文：面板默认折叠，用户先自己猜。
    pub translation: String,

    /// 引导用户做最后一步的问题；没有就留空。
    pub question: String,
}

impl Decoded {
    /// 一条有用的内容都没有。
    pub fn is_empty(&self) -> bool {
        self.situation.is_empty() && self.points.is_empty() && self.translation.is_empty()
    }
}
