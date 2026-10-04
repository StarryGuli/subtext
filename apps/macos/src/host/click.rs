//! 鼠标点候选：点击发生在候选窗的视图里，这里找到当前的输入控制器让它上屏。

use super::with;

/// 点了候选窗里的第 `row` 行（当前页第几个）。上屏碰应用客户端，所以在借用 `Host` 之外做。
pub fn click_candidate(row: usize) {
    let controller = with(|h| h.controller.clone()).flatten();
    if let Some(controller) = controller {
        controller.commit_clicked(row);
    }
}
