//! Coach 对两条通道的统一入口：把事件、按键、按钮分到对的那条上。

use super::action::CoachAction;
use super::{Coach, DECODE, Shown, WRITE, lane_for};

impl Coach {
    /// 开始新的一次：放进对应的通道，那条通道上一条出完结果的收进历史。
    pub(super) fn begin(&mut self, shown: Shown) {
        let index = lane_for(shown.mode);
        self.focus = index;
        self.lanes[index].begin(shown);
    }

    /// 取走两条通道的后台事件。
    pub(super) fn process_events(&mut self) {
        for index in 0..self.lanes.len() {
            for peer in self.lanes[index].process_events() {
                self.memory.remember_peer(&peer);
            }
        }
    }

    pub(super) fn expire(&mut self) {
        for lane in &mut self.lanes {
            lane.expire();
        }
    }

    /// 按键和「关闭」「上一条」这类按钮作用的通道：鼠标在哪块面板上就是哪块，否则是最近出内容的那块。
    pub(super) fn active(&self) -> usize {
        if let Some(index) = (0..self.lanes.len()).find(|&index| {
            self.lanes[index].is_visible() && self.lanes[index].panel.contains_mouse()
        }) {
            return index;
        }
        if self.lanes[self.focus].is_visible() {
            self.focus
        } else {
            1 - self.focus
        }
    }

    /// 动作该落在哪条通道：替换、复制选项只有组句 / 改稿有，展开译文只有解码有，其余看鼠标与焦点。
    pub(super) fn lane_of(&self, action: CoachAction) -> usize {
        match action {
            CoachAction::Replace(_)
            | CoachAction::Copy(_)
            | CoachAction::ReplaceEdited
            | CoachAction::CopyEdited
            | CoachAction::ReplaceAlternative(_) => WRITE,
            CoachAction::Reveal => DECODE,
            _ => self.active(),
        }
    }

    /// 展开解码的译文。
    pub fn reveal(&mut self) {
        self.lanes[DECODE].reveal();
    }

    /// 脚注里临时提示一句话。
    pub(super) fn set_note(&mut self, lane: usize, note: &str) {
        self.lanes[lane].set_note(note);
    }

    /// 翻到上一条或下一条，作用于当前通道。
    pub fn navigate(&mut self, delta: isize) {
        let index = self.active();
        self.lanes[index].navigate(delta);
    }

    /// 有别的解读可翻（当前通道）。
    pub fn can_navigate(&self) -> bool {
        self.lanes[self.active()].can_navigate()
    }

    /// 关掉当前通道的面板（Esc、「关闭」）。
    pub fn dismiss(&mut self) {
        let index = self.active();
        self.lanes[index].dismiss();
    }

    /// 两块面板都收起（切走输入法时）。
    pub(super) fn dismiss_all(&mut self) {
        for lane in &mut self.lanes {
            lane.dismiss();
        }
    }

    /// ⌥ + 数字对应的动作：只有组句 / 改稿通道有替换选项。
    pub fn digit_action(&mut self, digit: usize) -> Option<CoachAction> {
        let action = self.lanes[WRITE].digit_action(digit)?;
        self.focus = WRITE;
        Some(action)
    }

    pub fn is_showing(&self) -> bool {
        self.lanes.iter().any(|lane| lane.is_showing())
    }
}
