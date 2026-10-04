//! 把后台事件画到面板上：开始思考、出结果、出错；到时间自动收起。

use std::time::{Duration, Instant};

use objc2_app_kit::NSEvent;
use objc2_foundation::{NSRect, NSSize};
use subtext_coach::{CoachEvent, CoachOutput, Mode, friendly};

use super::action::CoachAction;
use super::doc::Doc;
use super::panel::PanelContent;
use super::{Coach, Shown};

/// 解码结果留多久：读英文、想一想要时间。
const DECODE_TTL: Duration = Duration::from_secs(600);

/// 组句结果留多久：用户多半很快就继续写了。
const COMPOSE_TTL: Duration = Duration::from_secs(120);

impl Coach {
    /// 开始新的一次：先显示「思考中」，结果到了再换。
    pub(super) fn begin(&mut self, mut shown: Shown) {
        // 没有光标位置的（复制触发的解码）固定在此刻的鼠标位置：后面每次重画都用它，面板不会跟着鼠标跑
        if shown.anchor == NSRect::ZERO {
            shown.anchor = NSRect::new(NSEvent::mouseLocation(), NSSize::new(0.0, 16.0));
        }
        self.shown = Some(shown);
        self.render();
    }

    /// 取走后台事件，只认最新那次请求的。
    pub(super) fn process_events(&mut self) {
        let Some(service) = &self.service else {
            return;
        };
        let events = service.poll();
        let mut changed = false;
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
                        self.memory.remember_peer(&shown.source);
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

    /// 展开解码的译文。
    pub fn reveal(&mut self) {
        if let Some(shown) = self.shown.as_mut() {
            shown.revealed = true;
        }
        self.render();
    }

    /// 脚注里临时提示一句话。
    pub(super) fn set_note(&mut self, note: &str) {
        if let Some(shown) = self.shown.as_mut() {
            shown.note = Some(note.to_owned());
        }
        self.render();
    }

    /// 按当前状态重画面板。
    pub(super) fn render(&mut self) {
        let Some(shown) = &self.shown else {
            self.panel.hide();
            return;
        };
        let content = content_for(
            shown,
            self.config.backend.label(),
            self.config.backend.is_local_cli(),
        );
        let anchor: NSRect = shown.anchor;
        self.panel.show(&content, anchor);
    }

    /// ⌥ + 数字对应的动作：组句时是第几个英文选项，改稿时 1 是修改版；没有对应的就是 `None`。
    pub fn digit_action(&self, digit: usize) -> Option<CoachAction> {
        if !self.panel.is_visible() || digit == 0 {
            return None;
        }
        if self.shown.as_ref()?.streaming {
            return None;
        }
        match self.shown.as_ref()?.output.as_ref()? {
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

    pub fn is_showing(&self) -> bool {
        self.shown.is_some() && self.panel.is_visible()
    }
}

/// 一次教练在面板里该是什么样。
fn content_for(shown: &Shown, backend: &str, slow: bool) -> PanelContent {
    let title = match shown.mode {
        Mode::Decode => "解码",
        Mode::Compose => "组句",
        Mode::Edit => "改稿",
        Mode::Screen => "屏幕",
    };
    let close = ("关闭".to_owned(), CoachAction::Close);
    let footer = shown
        .note
        .clone()
        .unwrap_or_else(|| format!("{backend} · Esc 关闭"));
    if let Some(message) = &shown.failure {
        return PanelContent {
            doc: Doc::failure(title, message),
            buttons: vec![close],
            footer,
        };
    }
    let Some(output) = &shown.output else {
        // 本机命令行后端要启动进程、逐字生成，通常 20–40 秒：说一声，免得以为卡死了
        let footer = if slow && shown.note.is_none() {
            format!("{backend} · 命令行后端较慢，约 20–40 秒；API 后端只要几秒")
        } else {
            footer
        };
        return PanelContent {
            doc: Doc::thinking(title),
            buttons: vec![close],
            footer,
        };
    };
    // 还在生成：内容没写完，不给替换 / 复制，只留关闭
    if shown.streaming {
        let doc = match output {
            CoachOutput::Decode(decoded) => Doc::decode(decoded, false),
            CoachOutput::Compose(composed) => Doc::compose(composed),
            CoachOutput::Edit(edited) => Doc::edit(edited),
            CoachOutput::Screen(_) => Doc::default(),
        };
        return PanelContent {
            doc,
            buttons: vec![close],
            footer: format!("{backend} · 生成中…"),
        };
    }
    let can_replace = shown.target.is_some();
    match output {
        CoachOutput::Decode(decoded) => {
            let mut buttons = Vec::new();
            if !shown.revealed && !decoded.translation.is_empty() {
                buttons.push(("显示译文".to_owned(), CoachAction::Reveal));
            }
            buttons.push(close);
            PanelContent {
                doc: Doc::decode(decoded, shown.revealed),
                buttons,
                footer,
            }
        }
        CoachOutput::Compose(composed) => {
            let mut buttons = Vec::new();
            for index in 0..composed.options.len().min(super::action::MAX_OPTIONS) {
                let title = if can_replace {
                    format!("替换 ⌥{}", index + 1)
                } else {
                    format!("复制 {}", index + 1)
                };
                let action = if can_replace {
                    CoachAction::Replace(index)
                } else {
                    CoachAction::Copy(index)
                };
                buttons.push((title, action));
            }
            if can_replace {
                let recommended = composed
                    .options
                    .iter()
                    .position(|option| option.recommended)
                    .unwrap_or(0);
                buttons.push(("复制推荐".to_owned(), CoachAction::Copy(recommended)));
            }
            buttons.push(close);
            PanelContent {
                doc: Doc::compose(composed),
                buttons,
                footer,
            }
        }
        CoachOutput::Edit(edited) => {
            let mut buttons = Vec::new();
            if can_replace {
                buttons.push(("替换 ⌥1".to_owned(), CoachAction::ReplaceEdited));
                for index in 0..edited
                    .alternatives
                    .len()
                    .min(super::action::MAX_OPTIONS - 1)
                {
                    buttons.push((
                        format!("更地道 ⌥{}", index + 2),
                        CoachAction::ReplaceAlternative(index),
                    ));
                }
            }
            buttons.push(("复制".to_owned(), CoachAction::CopyEdited));
            buttons.push(close);
            PanelContent {
                doc: Doc::edit(edited),
                buttons,
                footer,
            }
        }
        // 屏幕阅读有自己的请求与悬浮卡，不走这个面板
        CoachOutput::Screen(_) => PanelContent {
            doc: Doc::default(),
            buttons: vec![close],
            footer,
        },
    }
}

/// 失败原因转成给用户看的话：后台线程已经转好了，这里只是兜底。
#[allow(dead_code)]
fn describe(error: &subtext_coach::CoachError) -> String {
    friendly(error)
}
