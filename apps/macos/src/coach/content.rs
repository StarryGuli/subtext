//! 一次教练在面板里该是什么样：按种类、状态拼出文字与按钮，以及脚注开头的标签。

use subtext_coach::{CoachOutput, Mode};

use super::Shown;
use super::action::CoachAction;
use super::doc::Doc;
use super::panel::PanelContent;

/// 脚注开头的标签：哪种解读、第几条、原文开头，几个窗口来回看时靠它分清。
pub(super) fn label_for(shown: &Shown, current: usize, total: usize) -> String {
    let kind = match shown.mode {
        Mode::Decode => "解码",
        Mode::Compose => "组句",
        Mode::Edit => "改稿",
        Mode::Screen => "屏幕",
    };
    let snippet: String = shown.source.trim().chars().take(16).collect();
    let ellipsis = if shown.source.trim().chars().count() > 16 {
        "…"
    } else {
        ""
    };
    if total > 1 {
        format!("【{kind}】{}/{total} “{snippet}{ellipsis}”", current + 1)
    } else {
        format!("【{kind}】“{snippet}{ellipsis}”")
    }
}

/// 一次教练在面板里该是什么样。
pub(super) fn content_for(shown: &Shown, backend: &str, slow: bool) -> PanelContent {
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
            CoachOutput::Plain { text, .. } => Doc::plain(title, text),
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
            buttons.push(("复制全文".to_owned(), CoachAction::CopyAll));
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
        // 模型没按格式回：原文显示，能复制；点一下「复制」不会替换任何东西
        CoachOutput::Plain { text, .. } => PanelContent {
            doc: Doc::plain(title, text),
            buttons: vec![("复制".to_owned(), CoachAction::CopyPlain), close],
            footer: format!("{backend} · 模型没有按格式回复，直接显示原文"),
        },
        // 屏幕阅读有自己的请求与悬浮卡，不走这个面板
        CoachOutput::Screen(_) => PanelContent {
            doc: Doc::default(),
            buttons: vec![close],
            footer,
        },
    }
}
