use serde::{Deserialize, Serialize};

/// 任务栏「中 / 英」图标右键菜单里要交给 Server 办的项（中 / 英切换在 DLL 侧自己做）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndicatorCommand {
    /// 翻转当前模式的全角标点，与悬浮条上点「，。」一样。
    TogglePunctuation,

    /// 显示 / 隐藏悬浮状态条（`[status_bar] enabled`）。
    ToggleStatusBar,

    /// 打开设置程序。DLL 可能在 UWP 沙箱里起不了进程，交给 Server 起。
    OpenSettings,
}

/// 右键菜单打勾用的开关状态。DLL 不读配置文件（UWP 沙箱里读不到），由 Server 随
/// [`super::ServerMessage::ModeSync`] 每一拍带下来。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct IndicatorState {
    /// 中文模式下标点转全角（`[general] full_width_punctuation`）。
    pub full_width_punctuation: bool,

    /// 英文模式下标点转全角（`[general] english_full_width_punctuation`）。
    pub english_full_width_punctuation: bool,

    /// 悬浮状态条开着（`[status_bar] enabled`）。
    pub status_bar: bool,
}
