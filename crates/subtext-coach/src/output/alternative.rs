//! 改稿里给出的另一种（通常更地道的）说法。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Alternative {
    /// 可直接替换的英文。
    pub text: String,

    /// 好在哪：更自然、更简洁、更符合场合。
    pub why: String,
}
