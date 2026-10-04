//! 把已上屏的中文替换成英文：先核对应用里那段文字没变，再动手；对不上就改成复制。

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::NSRange;
use subtext_coach::CoachOutput;

use super::action::CoachAction;
use super::{Coach, clipboard};
use crate::imk::TextClient;

/// 要在应用里做的一次替换。碰客户端，所以在借用 Host 之外执行。
pub struct ReplacePlan {
    client: Retained<AnyObject>,

    range: NSRange,

    expected: String,

    replacement: String,
}

/// 替换的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaceOutcome {
    Replaced,

    /// 应用里那段文字已经变了，没有动它。
    Changed,
}

impl ReplacePlan {
    pub fn execute(&self) -> ReplaceOutcome {
        let client = TextClient::new(&self.client);
        match client.text_in_range(self.range) {
            Some(current) if current == self.expected => {
                client.replace_range(&self.replacement, self.range);
                ReplaceOutcome::Replaced
            }
            _ => ReplaceOutcome::Changed,
        }
    }

    pub fn replacement(&self) -> &str {
        &self.replacement
    }
}

impl Coach {
    /// 该动作要替换应用里的文字时，给出替换计划；不是替换动作或没有可替换的位置返回 `None`。
    pub fn replace_plan(&self, action: CoachAction) -> Option<ReplacePlan> {
        let shown = self.shown.as_ref().filter(|shown| !shown.streaming)?;
        let target = shown.target.as_ref()?;
        let replacement = match (action, shown.output.as_ref()?) {
            (CoachAction::Replace(index), CoachOutput::Compose(composed)) => {
                composed.options.get(index)?.text.trim().to_owned()
            }
            (CoachAction::ReplaceEdited, CoachOutput::Edit(edited)) => {
                edited.corrected.trim().to_owned()
            }
            (CoachAction::ReplaceAlternative(index), CoachOutput::Edit(edited)) => {
                edited.alternatives.get(index)?.text.trim().to_owned()
            }
            _ => return None,
        };
        (!replacement.is_empty()).then(|| ReplacePlan {
            client: target.client.clone(),
            range: target.range,
            expected: target.expected.clone(),
            replacement,
        })
    }

    /// 替换完成后的收尾：成功就收起面板；没动成就把英文放进剪贴板并告诉用户。
    pub fn after_replace(&mut self, plan: &ReplacePlan, outcome: ReplaceOutcome) {
        match outcome {
            ReplaceOutcome::Replaced => self.dismiss(),
            ReplaceOutcome::Changed => {
                let count = clipboard::write(plan.replacement());
                self.clipboard.mark_own(count);
                self.set_note("原文已经变动，没有替换；英文已复制，⌘V 粘贴");
            }
        }
    }

    /// 复制某个英文选项（或修改版）到剪贴板。
    pub fn copy(&mut self, action: CoachAction) {
        let Some(output) = self
            .shown
            .as_ref()
            .filter(|shown| !shown.streaming)
            .and_then(|shown| shown.output.as_ref())
        else {
            return;
        };
        let text = match (action, output) {
            (CoachAction::Copy(index), CoachOutput::Compose(composed)) => composed
                .options
                .get(index)
                .map(|option| option.text.trim().to_owned()),
            (CoachAction::CopyEdited, CoachOutput::Edit(edited)) => {
                Some(edited.corrected.trim().to_owned())
            }
            _ => None,
        };
        let Some(text) = text.filter(|text| !text.is_empty()) else {
            return;
        };
        let count = clipboard::write(&text);
        self.clipboard.mark_own(count);
        self.set_note("已复制");
    }
}
