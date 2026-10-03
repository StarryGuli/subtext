//! 面板上的按钮动作；用按钮的 tag 在 AppKit 与 Rust 之间来回。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoachAction {
    /// 关掉面板。
    Close,

    /// 展开解码的译文。
    Reveal,

    /// 用第 n 个英文选项替换刚输入的中文（从 0 数）。
    Replace(usize),

    /// 复制第 n 个英文选项。
    Copy(usize),

    /// 用修改后的版本替换原文。
    ReplaceEdited,

    /// 复制修改后的版本。
    CopyEdited,
}

/// 选项最多这么多个，tag 编码依赖它。
pub const MAX_OPTIONS: usize = 3;

impl CoachAction {
    pub fn tag(self) -> isize {
        match self {
            Self::Close => 1,
            Self::Reveal => 2,
            Self::ReplaceEdited => 3,
            Self::CopyEdited => 4,
            Self::Replace(index) => 10 + index.min(MAX_OPTIONS - 1) as isize,
            Self::Copy(index) => 20 + index.min(MAX_OPTIONS - 1) as isize,
        }
    }

    pub fn from_tag(tag: isize) -> Option<Self> {
        let index = |base: isize| {
            usize::try_from(tag - base)
                .ok()
                .filter(|i| *i < MAX_OPTIONS)
        };
        match tag {
            1 => Some(Self::Close),
            2 => Some(Self::Reveal),
            3 => Some(Self::ReplaceEdited),
            4 => Some(Self::CopyEdited),
            10..=19 => index(10).map(Self::Replace),
            20..=29 => index(20).map(Self::Copy),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_round_trip() {
        let all = [
            CoachAction::Close,
            CoachAction::Reveal,
            CoachAction::ReplaceEdited,
            CoachAction::CopyEdited,
            CoachAction::Replace(0),
            CoachAction::Replace(2),
            CoachAction::Copy(1),
        ];
        for action in all {
            assert_eq!(CoachAction::from_tag(action.tag()), Some(action));
        }
        assert_eq!(CoachAction::from_tag(0), None);
        assert_eq!(CoachAction::from_tag(13), None);
    }
}
