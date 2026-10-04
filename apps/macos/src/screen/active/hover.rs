//! 悬浮卡与状态条：鼠标停在哪一块就在旁边弹出那一块的译文；停得久了换成完整解读。

use std::time::Duration;

use objc2_foundation::{NSPoint, NSRect, NSSize};
use subtext_coach::CoachOutput;
use subtext_coach::screen::block_at;

use super::Active;
use crate::coach::{CoachPanel, Doc, PanelContent};
use crate::screen::desktop::{mouse_cg, primary_height};
use crate::screen::geometry;

/// 鼠标在一块上停多久才弹出悬浮卡：划过去不弹。
const HOVER_DELAY: Duration = Duration::from_millis(350);

/// 停多久换成完整解读（要已经预解码好）。
const FULL_DELAY: Duration = Duration::from_millis(1200);

/// 鼠标离开块之后卡片再留多久：让鼠标有时间移到卡片上。
const LEAVE_GRACE: Duration = Duration::from_millis(450);

/// 鼠标离块边缘这么近（点）也算指着它。
const HOVER_SLACK: f32 = 3.0;

pub(in crate::screen) const PILL_WIDTH: f64 = 230.0;

impl Active {
    /// 鼠标停在哪一块、停了多久；够久了就弹出那一块的悬浮卡，离开了稍等一下再收起。
    pub(in crate::screen) fn update_card(&mut self, card: &mut CoachPanel) {
        if self.leave_since.is_some() && card.contains_mouse() {
            self.leave_since = None;
            return;
        }
        let (x, y) = mouse_cg();
        let Some(block) = block_at(&self.blocks, x, y, HOVER_SLACK).cloned() else {
            if card.is_visible() || self.card_state.is_some() {
                let left = *self.leave_since.get_or_insert_with(std::time::Instant::now);
                if left.elapsed() < LEAVE_GRACE || card.contains_mouse() {
                    return;
                }
            }
            self.hover_key = None;
            self.leave_since = None;
            self.card_state = None;
            card.hide();
            return;
        };
        self.leave_since = None;
        if self.hover_key.as_deref() != Some(block.key.as_str()) {
            self.hover_key = Some(block.key.clone());
            self.hover_since = std::time::Instant::now();
            self.card_state = None;
            card.hide();
            return;
        }
        let dwell = self.hover_since.elapsed();
        if dwell < HOVER_DELAY {
            return;
        }
        let full = match self.decoded.get(&block.key) {
            Some(CoachOutput::Decode(decoded)) if dwell >= FULL_DELAY => Some(decoded.clone()),
            _ => None,
        };
        let state = (
            block.key.clone(),
            block.analysis.is_some(),
            block.pending,
            full.is_some(),
        );
        if self.card_state.as_ref() == Some(&state) {
            return;
        }
        let eligible = self
            .gate
            .allow_screen_block(&block.text, self.app().as_deref());
        let doc = match (&full, &block.analysis) {
            (Some(decoded), _) => Doc::decode(decoded, true),
            (None, Some(analysis)) => Doc::screen_card(&analysis.translation, &analysis.note),
            (None, None) if !eligible => {
                self.card_state = Some(state);
                card.hide();
                return;
            }
            (None, None) if block.pending => Doc::screen_status("翻译中…"),
            (None, None) => Doc::screen_status(self.last_error.as_deref().unwrap_or("等待翻译…")),
        };
        self.card_state = Some(state);
        let rect = block.rect;
        let anchor = NSRect::new(
            NSPoint::new(
                f64::from(rect.x),
                geometry::cg_to_cocoa_y(rect.y, rect.height, primary_height()),
            ),
            NSSize::new(f64::from(rect.width), f64::from(rect.height)),
        );
        card.show(
            &PanelContent {
                doc,
                buttons: Vec::new(),
                footer: String::new(),
            },
            anchor,
        );
    }

    /// 状态条：目标窗口右上角一个小标记，告诉用户屏幕正在被读（隐私上必须一直看得见）。
    pub(in crate::screen) fn update_pill(&mut self, pill: &mut CoachPanel) {
        if self.last_pill.elapsed() < Duration::from_millis(700) {
            return;
        }
        self.last_pill = std::time::Instant::now();
        let text = match &self.last_error {
            _ if self.paused => "● 屏幕阅读已暂停（正在输入密码）".to_owned(),
            _ if self.gone_since.is_some() => "● 屏幕阅读 · 窗口暂时看不到，等它回来".to_owned(),
            Some(error) => format!("● 屏幕阅读 · {error}"),
            None => format!(
                "● 屏幕阅读中 · {} · 已翻译 {} 条",
                self.target.label(),
                self.translated
            ),
        };
        let bounds = self.bounds;
        let top_left = NSPoint::new(
            f64::from(bounds.x + bounds.width) - PILL_WIDTH - 8.0,
            geometry::cg_to_cocoa_y(bounds.y, 0.0, primary_height()) - 8.0,
        );
        pill.show_at(
            &PanelContent {
                doc: Doc::screen_status(&text),
                buttons: Vec::new(),
                footer: String::new(),
            },
            top_left,
        );
    }
}
