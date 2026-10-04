//! 一条通道：一块面板、一个后台线程（最新请求优先）、正显示的解读和翻得回去的之前几条。
//!
//! 解码一条、组句 / 改稿一条，两条互不顶替，所以两种结果能同时在屏幕上，也能同时在后台生成。

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use objc2_app_kit::NSEvent;
use objc2_foundation::{NSRect, NSSize};
use subtext_coach::{CoachEvent, CoachOutput, CoachService, Mode};

use super::action::CoachAction;
use super::content::{content_for, label_for};
use super::panel::CoachPanel;
use super::{PAST_LIMIT, Shown};

/// 解码结果留多久：读英文、想一想要时间。
const DECODE_TTL: Duration = Duration::from_secs(600);

/// 组句结果留多久：用户多半很快就继续写了。
const COMPOSE_TTL: Duration = Duration::from_secs(120);

pub(super) struct Lane {
    pub(super) panel: CoachPanel,

    /// 教练开着才有；配置变了整个换掉，旧线程自己退出。
    pub(super) service: Option<CoachService>,

    shown: Option<Shown>,

    /// 之前看过的解读，旧的在前；只收出完结果的，替换位置不留（应用里的文字早就变了）。
    past: VecDeque<Shown>,

    /// 面板正在看 `past` 里的第几条；`None` 是最新的那条。
    view: Option<usize>,

    /// 脚注里的后端名，以及后端是不是本机命令行（慢，要在等待时说一声）。
    backend: String,

    slow: bool,
}

impl Lane {
    pub(super) fn new(panel: CoachPanel) -> Self {
        Self {
            panel,
            service: None,
            shown: None,
            past: VecDeque::new(),
            view: None,
            backend: String::new(),
            slow: false,
        }
    }

    pub(super) fn set_backend(&mut self, backend: &str, slow: bool) {
        backend.clone_into(&mut self.backend);
        self.slow = slow;
    }

    /// 开始新的一次：先显示「思考中」，结果到了再换；上一条出完结果的收进历史。
    pub(super) fn begin(&mut self, mut shown: Shown) {
        // 没有光标位置的（复制 / 按压触发的解码）固定在此刻的鼠标位置：后面每次重画都用它，面板不会跟着鼠标跑
        if shown.anchor == NSRect::ZERO {
            shown.anchor = NSRect::new(NSEvent::mouseLocation(), NSSize::new(0.0, 16.0));
        }
        if let Some(old) = self.shown.take() {
            self.archive(old);
        }
        self.view = None;
        self.shown = Some(shown);
        self.render();
    }

    /// 取走后台事件，只认最新那次请求的。返回刚解码完的对方消息原文，给组句接语气用。
    pub(super) fn process_events(&mut self) -> Vec<String> {
        let Some(service) = &self.service else {
            return Vec::new();
        };
        let events = service.poll();
        let mut changed = false;
        let mut peers = Vec::new();
        for event in events {
            let Some(shown) = self.shown.as_mut().filter(|shown| shown.id == event.id()) else {
                continue;
            };
            match event {
                CoachEvent::Started { .. } => {}
                CoachEvent::Partial { output, .. } => {
                    shown.output = Some(output);
                    shown.streaming = true;
                    changed = true;
                }
                CoachEvent::Finished { output, .. } => {
                    shown.streaming = false;
                    if let (CoachOutput::Decode(_), true) = (&output, shown.mode == Mode::Decode) {
                        peers.push(shown.source.clone());
                    }
                    shown.output = Some(output);
                    shown.shown_at = Instant::now();
                    changed = true;
                }
                CoachEvent::Failed { message, .. } => {
                    shown.failure = Some(message);
                    changed = true;
                }
            }
        }
        if changed {
            self.render();
        }
        peers
    }

    /// 到时间了就收起面板；鼠标停在面板上（正在读、正要点按钮）时重新计时。
    pub(super) fn expire(&mut self) {
        if self.panel.is_visible()
            && self.panel.contains_mouse()
            && let Some(shown) = self.shown.as_mut()
        {
            shown.shown_at = Instant::now();
        }
        let expired = self.shown.as_ref().is_some_and(|shown| {
            let ttl = match shown.mode {
                Mode::Compose => COMPOSE_TTL,
                _ => DECODE_TTL,
            };
            shown.output.is_some() && shown.shown_at.elapsed() > ttl
        });
        if expired {
            self.dismiss();
        }
    }

    /// 关掉面板；正显示的收进历史（出完结果的）。
    pub(super) fn dismiss(&mut self) {
        self.panel.hide();
        if let Some(shown) = self.shown.take() {
            self.archive(shown);
        }
        self.view = None;
    }

    /// 关掉面板并忘掉所有历史（教练被关掉时）。
    pub(super) fn reset(&mut self) {
        self.dismiss();
        self.past.clear();
    }

    fn archive(&mut self, mut shown: Shown) {
        if shown.output.is_none() || shown.failure.is_some() || shown.streaming {
            return;
        }
        shown.target = None;
        shown.note = None;
        self.past.push_back(shown);
        while self.past.len() > PAST_LIMIT {
            self.past.pop_front();
        }
    }

    /// 面板正在看的那条：翻回去时是之前的，否则是最新的。
    pub(super) fn viewed(&self) -> Option<&Shown> {
        match self.view {
            Some(index) => self.past.get(index),
            None => self.shown.as_ref(),
        }
    }

    fn viewed_mut(&mut self) -> Option<&mut Shown> {
        match self.view {
            Some(index) => self.past.get_mut(index),
            None => self.shown.as_mut(),
        }
    }

    /// 可翻看的总条数（含最新的那条）与当前是第几条（从 0 数）。
    fn position(&self) -> (usize, usize) {
        let total = self.past.len() + usize::from(self.shown.is_some());
        let current = self.view.unwrap_or(total.saturating_sub(1));
        (current, total)
    }

    pub(super) fn can_navigate(&self) -> bool {
        self.position().1 > 1
    }

    /// 翻到上一条（`delta` 为负）或下一条；到头不动。
    pub(super) fn navigate(&mut self, delta: isize) {
        let (current, total) = self.position();
        if total < 2 {
            return;
        }
        let next = current.saturating_add_signed(delta).min(total - 1);
        self.view = if next == total - 1 && self.shown.is_some() {
            None
        } else {
            Some(next)
        };
        self.render();
    }

    /// 展开解码的译文。
    pub(super) fn reveal(&mut self) {
        if let Some(shown) = self.viewed_mut() {
            shown.revealed = true;
        }
        self.render();
    }

    /// 脚注里临时提示一句话。
    pub(super) fn set_note(&mut self, note: &str) {
        if let Some(shown) = self.viewed_mut() {
            shown.note = Some(note.to_owned());
        }
        self.render();
    }

    /// 按当前状态重画面板。
    pub(super) fn render(&mut self) {
        let Some(shown) = self.viewed() else {
            self.panel.hide();
            return;
        };
        let mut content = content_for(shown, &self.backend, self.slow);
        let anchor: NSRect = shown.anchor;
        let (current, total) = self.position();
        content.footer = format!("{} · {}", label_for(shown, current, total), content.footer);
        if total > 1 {
            let close = content.buttons.pop();
            if current > 0 {
                content
                    .buttons
                    .push(("‹ 上一条".to_owned(), CoachAction::Previous));
            }
            if current + 1 < total {
                content
                    .buttons
                    .push(("下一条 ›".to_owned(), CoachAction::Next));
            }
            content.buttons.extend(close);
        }
        self.panel.show(&content, anchor);
    }

    /// ⌥ + 数字对应的动作：组句时是第几个英文选项，改稿时 1 是修改版；没有对应的就是 `None`。
    pub(super) fn digit_action(&self, digit: usize) -> Option<CoachAction> {
        if !self.panel.is_visible() || digit == 0 {
            return None;
        }
        let shown = self.viewed()?;
        if shown.streaming {
            return None;
        }
        match shown.output.as_ref()? {
            CoachOutput::Compose(composed)
                if digit <= composed.options.len().min(super::action::MAX_OPTIONS) =>
            {
                Some(CoachAction::Replace(digit - 1))
            }
            CoachOutput::Edit(_) if digit == 1 => Some(CoachAction::ReplaceEdited),
            CoachOutput::Edit(edited)
                if (2..=1 + edited
                    .alternatives
                    .len()
                    .min(super::action::MAX_OPTIONS - 1))
                    .contains(&digit) =>
            {
                Some(CoachAction::ReplaceAlternative(digit - 2))
            }
            _ => None,
        }
    }

    pub(super) fn is_showing(&self) -> bool {
        self.shown.is_some() && self.panel.is_visible()
    }

    pub(super) fn is_visible(&self) -> bool {
        self.panel.is_visible()
    }
}
