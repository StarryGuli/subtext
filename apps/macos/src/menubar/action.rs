use objc2_foundation::NSInteger;
use subtext_core::FuzzyRules;

/// 模糊音条目的 tag 起点，后面加规则在 [`FuzzyRules::NAMES`] 里的下标。
const FUZZY_TAG_BASE: NSInteger = 100;

/// 菜单能触发的动作。编码进 NSMenuItem 的 tag，派发时再解出来。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// 开关云联想（写 `[predict] enabled`）。
    ToggleCloud,

    /// 开关双语教练（写 `[coach] enabled`）。
    ToggleCoach,

    /// 打开偏好设置并跳到「模糊音」页：菜单里勾一项就收起，要连着改多条在设置里改。
    OpenFuzzySettings,

    /// 开关一条模糊音规则，值是 [`FuzzyRules::NAMES`] 的下标。
    ToggleFuzzy(usize),

    /// 打开偏好设置窗口。
    OpenPreferences,

    /// 在访达里打开日志目录。
    OpenLogs,

    /// 打开下载页（菜单里「有新版本」那一行）。
    OpenDownload,
}

impl MenuAction {
    pub fn tag(self) -> NSInteger {
        match self {
            Self::ToggleCloud => 1,
            Self::OpenPreferences => 2,
            Self::OpenLogs => 3,
            Self::OpenDownload => 4,
            Self::ToggleCoach => 5,
            Self::OpenFuzzySettings => 6,
            Self::ToggleFuzzy(index) => FUZZY_TAG_BASE + index as NSInteger,
        }
    }

    pub fn from_tag(tag: NSInteger) -> Option<Self> {
        Some(match tag {
            1 => Self::ToggleCloud,
            2 => Self::OpenPreferences,
            3 => Self::OpenLogs,
            4 => Self::OpenDownload,
            5 => Self::ToggleCoach,
            6 => Self::OpenFuzzySettings,
            _ => {
                let index = usize::try_from(tag.checked_sub(FUZZY_TAG_BASE)?).ok()?;
                (index < FuzzyRules::NAMES.len()).then_some(Self::ToggleFuzzy(index))?
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_round_trip() {
        let all = [
            MenuAction::ToggleCloud,
            MenuAction::OpenPreferences,
            MenuAction::OpenLogs,
            MenuAction::OpenDownload,
            MenuAction::ToggleCoach,
            MenuAction::OpenFuzzySettings,
            MenuAction::ToggleFuzzy(0),
            MenuAction::ToggleFuzzy(FuzzyRules::NAMES.len() - 1),
        ];
        for action in all {
            assert_eq!(MenuAction::from_tag(action.tag()), Some(action));
        }
        assert_eq!(MenuAction::from_tag(0), None);
        assert_eq!(
            MenuAction::from_tag(FUZZY_TAG_BASE + FuzzyRules::NAMES.len() as NSInteger),
            None
        );
        assert_eq!(MenuAction::from_tag(-1), None);
    }
}
