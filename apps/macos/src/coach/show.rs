//! 把后台事件画到面板上：开始思考、出结果、出错；到时间自动收起。

use std::time::{Duration, Instant};

use objc2_foundation::NSRect;
use subtext_coach::{CoachEvent, CoachOutput, Mode, friendly};

use super::action::CoachAction;
use super::doc::Doc;
use super::panel::PanelContent;
use super::{Coach, Shown};

/// 解码结果留多久：读英文、想一想要时间。
const DECODE_TTL: Duration = Duration::from_secs(120);

/// 组句结果留多久：用户多半很快就继续写了。
const COMPOSE_TTL: Duration = Duration::from_secs(45);

impl Coach {
    /// 开始新的一次：先显示「思考中」，结果到了再换。
    pub(super) fn begin(&mut self, shown: Shown) {
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
                CoachEvent::Finished { output, .. } => {
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

    /// 到时间了就收起面板。
    pub(super) fn expire(&mut self) {
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
        let content = content_for(shown, self.config.backend.label());
        let anchor: NSRect = shown.anchor;
        self.panel.show(&content, anchor);
    }

    /// 面板是否正显示组句结果、且带有可替换的选项（⌥1–3 才有意义）。
    pub fn compose_options(&self) -> usize {
        match self.shown.as_ref().and_then(|shown| shown.output.as_ref()) {
            Some(CoachOutput::Compose(composed)) if self.panel.is_visible() => composed.options.len(),
            _ => 0,
        }
    }

    pub fn is_showing(&self) -> bool {
        self.shown.is_some() && self.panel.is_visible()
    }
}

/// 一次教练在面板里该是什么样。
fn content_for(shown: &Shown, backend: &str) -> PanelContent {
    let title = match shown.mode {
        Mode::Decode => "解码",
        Mode::Compose => "组句",
        Mode::Edit => "改稿",
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
        return PanelContent {
            doc: Doc::thinking(title),
            buttons: vec![close],
            footer,
        };
    };
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
        CoachOutput::Edit(edited) => PanelContent {
            doc: Doc::edit(edited),
            buttons: vec![("复制".to_owned(), CoachAction::CopyEdited), close],
            footer,
        },
    }
}

/// 失败原因转成给用户看的话：后台线程已经转好了，这里只是兜底。
#[allow(dead_code)]
fn describe(error: &subtext_coach::CoachError) -> String {
    friendly(error)
}
