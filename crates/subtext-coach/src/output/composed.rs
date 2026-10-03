//! 组句结果：按语境给出的英文选项，加上「为什么这样说」。

use serde::{Deserialize, Serialize};

/// 一个可直接发送的英文版本。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ComposeOption {
    /// `casual` / `neutral` / `formal`。
    pub register: String,

    pub text: String,

    /// 模型认为最合适当前场合的那一个。
    pub recommended: bool,
}

/// 一个教学点：中文怎么说 → 英文怎么说 → 对应关系与原因。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ComposePoint {
    pub zh: String,

    pub en: String,

    /// `direct`（直接对应）/ `partial`（语感不同）/ `none`（没有对应）。
    pub mapping: String,

    pub why: String,
}

/// 组句模式的完整输出。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Composed {
    /// 推断的场合与对象，一句话；用户据此判断推断对不对。
    pub context: String,

    pub options: Vec<ComposeOption>,

    pub points: Vec<ComposePoint>,

    /// 直译会出错或失礼的地方。
    pub traps: Vec<String>,
}

impl Composed {
    /// 推荐的选项，没标就取第一个。
    pub fn recommended(&self) -> Option<&ComposeOption> {
        self.options
            .iter()
            .find(|option| option.recommended)
            .or_else(|| self.options.first())
    }

    pub fn is_empty(&self) -> bool {
        self.options
            .iter()
            .all(|option| option.text.trim().is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommended_falls_back_to_first() {
        let composed = Composed {
            options: vec![
                ComposeOption {
                    text: "a".into(),
                    ..Default::default()
                },
                ComposeOption {
                    text: "b".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(composed.recommended().unwrap().text, "a");
        let marked = Composed {
            options: vec![
                ComposeOption {
                    text: "a".into(),
                    ..Default::default()
                },
                ComposeOption {
                    text: "b".into(),
                    recommended: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(marked.recommended().unwrap().text, "b");
    }
}
