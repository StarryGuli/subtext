//! 解码结果：逐句译文（主体）、少量值得学的点、一句话语气。

use serde::{Deserialize, Serialize};

/// 原文的一句话和它的中文意思：用户拿到消息先要看懂每一句。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DecodeLine {
    /// 原文里的一句，保持原样。
    pub en: String,

    /// 这一句的中文意思，口语译成口语。
    pub zh: String,

    /// 这一句里缩写、俚语、套话的一句提示；没有就留空。
    pub note: String,
}

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

    /// 逐句译文，面板的主体；一条消息按句拆开，不跳句。
    pub lines: Vec<DecodeLine>,

    pub points: Vec<DecodePoint>,

    pub tone: Tone,

    /// 整段的中文意思，一两句话；`lines` 已覆盖时可留空。
    pub translation: String,
}

impl Decoded {
    /// 一条有用的内容都没有。
    pub fn is_empty(&self) -> bool {
        self.situation.is_empty()
            && self.lines.is_empty()
            && self.points.is_empty()
            && self.translation.is_empty()
    }
}
