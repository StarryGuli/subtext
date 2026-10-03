//! 改稿结果：修正版在前，逐条说明，并点出做对的地方。

use serde::{Deserialize, Serialize};

/// 一处修改。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Fix {
    pub from: String,

    pub to: String,

    /// `spelling` / `word` / `grammar` / `register`。
    pub kind: String,

    pub why: String,
}

/// 改稿模式的完整输出。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Edited {
    pub corrected: String,

    pub fixes: Vec<Fix>,

    /// 用户写得自然、值得保留与强化的地方。
    pub kept: Vec<String>,

    /// 反复出现的错误模式（冠词、单复数、时态之类），没有就留空。
    pub pattern: String,
}

impl Edited {
    pub fn is_empty(&self) -> bool {
        self.corrected.trim().is_empty()
    }
}
