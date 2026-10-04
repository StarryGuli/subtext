//! 教练输出的展示文档：把结构化结果排成带样式的行，与 AppKit 无关，好测。
//!
//! 层级沿用候选窗的原则：原文与英文最醒目，说明弱一档，辅助信息永远不抢正文。

use subtext_coach::{Composed, Decoded, Edited};

/// 一段文字的样式；具体字体与颜色在 [`super::attributed`] 里按系统外观取。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// 小标题（模式名、场合）。
    Title,

    /// 正文。
    Body,

    /// 英文短语 / 可直接发送的英文：加粗、醒目。
    Phrase,

    /// 说明：比正文弱一档。
    Dim,

    /// 橙：俚语、缩写、生词。
    Orange,

    /// 青：软化语、套话。
    Teal,

    /// 鼠尾草绿底：语气与潜台词。
    Highlight,

    /// 失败的提示。
    Warn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub text: String,

    pub style: Style,
}

/// 一个段落：若干样式段，`gap` 为与上一段之间多留一段空隙。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub runs: Vec<Run>,

    pub gap: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Doc {
    pub lines: Vec<Line>,
}

impl Doc {
    fn push(&mut self, gap: bool, runs: Vec<(Style, String)>) {
        let runs: Vec<Run> = runs
            .into_iter()
            .filter(|(_, text)| !text.is_empty())
            .map(|(style, text)| Run { text, style })
            .collect();
        if !runs.is_empty() {
            self.lines.push(Line { runs, gap });
        }
    }

    /// 全文纯文本，测试用。
    #[cfg(test)]
    pub fn plain_text(&self) -> String {
        self.lines
            .iter()
            .map(|line| {
                line.runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 解码。`reveal` 为假时译文折叠，让用户先自己猜。
    pub fn decode(decoded: &Decoded, reveal: bool) -> Self {
        let mut doc = Self::default();
        doc.push(
            false,
            vec![
                (Style::Title, "解码  ".to_owned()),
                (Style::Body, decoded.situation.clone()),
            ],
        );
        for point in &decoded.points {
            let tag_style = match point.kind.as_str() {
                "slang" | "abbr" => Style::Orange,
                "hedge" => Style::Teal,
                _ => Style::Dim,
            };
            doc.push(
                true,
                vec![
                    (Style::Phrase, point.phrase.clone()),
                    (tag_style, format!("   {}", kind_label(&point.kind))),
                ],
            );
            let meaning = match (point.literal.is_empty(), point.meaning.is_empty()) {
                (false, false) => format!("{} → {}", point.literal, point.meaning),
                (true, _) => point.meaning.clone(),
                (_, true) => point.literal.clone(),
            };
            doc.push(false, vec![(Style::Body, meaning)]);
            if !point.zh.is_empty() {
                doc.push(false, vec![(Style::Dim, format!("中文：{}", point.zh))]);
            }
            if !point.usage.is_empty() {
                doc.push(
                    false,
                    vec![
                        (Style::Dim, "你可以说：".to_owned()),
                        (Style::Phrase, point.usage.clone()),
                    ],
                );
            }
        }
        let tone = &decoded.tone;
        if !tone.register.is_empty() || !tone.subtext.is_empty() {
            doc.push(
                true,
                vec![(
                    Style::Highlight,
                    format!("语气 · {}　{}", tone.register, tone.subtext)
                        .trim()
                        .to_owned(),
                )],
            );
        }
        if !tone.contrast.is_empty() {
            doc.push(
                false,
                vec![(Style::Dim, format!("对照：{}", tone.contrast))],
            );
        }
        if !decoded.question.is_empty() {
            doc.push(
                true,
                vec![(Style::Dim, format!("想一想：{}", decoded.question))],
            );
        }
        if reveal && !decoded.translation.is_empty() {
            doc.push(
                true,
                vec![
                    (Style::Dim, "译文　".to_owned()),
                    (Style::Body, decoded.translation.clone()),
                ],
            );
        }
        doc
    }

    /// 组句。选项的英文放在最醒目的位置；`recommended` 的加星。
    pub fn compose(composed: &Composed) -> Self {
        let mut doc = Self::default();
        doc.push(
            false,
            vec![
                (Style::Title, "组句  ".to_owned()),
                (Style::Dim, composed.context.clone()),
            ],
        );
        for (index, option) in composed.options.iter().enumerate() {
            let star = if option.recommended { "★ " } else { "" };
            doc.push(
                true,
                vec![(
                    Style::Dim,
                    format!(
                        "{}  {}[{}]  ",
                        index + 1,
                        star,
                        register_label(&option.register)
                    ),
                )],
            );
            doc.push(false, vec![(Style::Phrase, compact(option.text.trim()))]);
        }
        for point in &composed.points {
            doc.push(
                true,
                vec![
                    (Style::Body, format!("{} → ", point.zh)),
                    (Style::Phrase, point.en.clone()),
                    (
                        mapping_style(&point.mapping),
                        format!("   {}", mapping_label(&point.mapping)),
                    ),
                ],
            );
            doc.push(false, vec![(Style::Dim, point.why.clone())]);
        }
        for trap in &composed.traps {
            doc.push(true, vec![(Style::Orange, format!("⚠ {trap}"))]);
        }
        doc
    }

    /// 改稿。
    pub fn edit(edited: &Edited) -> Self {
        let mut doc = Self::default();
        doc.push(false, vec![(Style::Title, "改稿".to_owned())]);
        doc.push(
            false,
            vec![(Style::Phrase, edited.corrected.trim().to_owned())],
        );
        if !edited.translation.is_empty() {
            doc.push(
                false,
                vec![
                    (Style::Dim, "意思　".to_owned()),
                    (Style::Body, edited.translation.clone()),
                ],
            );
        }
        for fix in &edited.fixes {
            doc.push(
                true,
                vec![
                    (Style::Dim, format!("{} → ", fix.from)),
                    (Style::Phrase, fix.to.clone()),
                    (Style::Dim, format!("   {}", fix_label(&fix.kind))),
                ],
            );
            doc.push(false, vec![(Style::Dim, fix.why.clone())]);
        }
        for alternative in &edited.alternatives {
            doc.push(
                true,
                vec![
                    (Style::Orange, "更地道  ".to_owned()),
                    (Style::Phrase, alternative.text.trim().to_owned()),
                ],
            );
            doc.push(false, vec![(Style::Dim, alternative.why.clone())]);
        }
        for kept in &edited.kept {
            doc.push(true, vec![(Style::Teal, format!("✓ {kept}"))]);
        }
        if !edited.pattern.is_empty() {
            doc.push(
                true,
                vec![(Style::Highlight, format!("反复出现 · {}", edited.pattern))],
            );
        }
        doc
    }

    /// 屏幕阅读悬浮卡：译文最醒目，下面一行弱一档的提示。
    pub fn screen_card(translation: &str, note: &str) -> Self {
        let mut doc = Self::default();
        doc.push(false, vec![(Style::Body, translation.to_owned())]);
        doc.push(true, vec![(Style::Orange, note.to_owned())]);
        doc
    }

    /// 悬浮卡上的一句状态（解析中、跳过的原因）。
    pub fn screen_status(text: &str) -> Self {
        let mut doc = Self::default();
        doc.push(false, vec![(Style::Dim, text.to_owned())]);
        doc
    }

    /// 等待后端时显示的占位。
    pub fn thinking(title: &str) -> Self {
        let mut doc = Self::default();
        doc.push(
            false,
            vec![
                (Style::Title, format!("{title}  ")),
                (Style::Dim, "思考中…".to_owned()),
            ],
        );
        doc
    }

    /// 失败时显示的一句话。
    pub fn failure(title: &str, message: &str) -> Self {
        let mut doc = Self::default();
        doc.push(false, vec![(Style::Title, format!("{title}  "))]);
        doc.push(false, vec![(Style::Warn, message.to_owned())]);
        doc
    }
}

/// 多行英文（邮件正文）里的空行压成一个换行：面板寸土寸金，段落仍看得出来。
fn compact(text: &str) -> String {
    let mut lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    lines.retain(|line| !line.is_empty());
    lines.join("\n")
}

fn kind_label(kind: &str) -> &'static str {
    match kind {
        "slang" => "俚语",
        "abbr" => "缩写",
        "hedge" => "软化 / 套话",
        "culture" => "文化",
        "fixed" => "固定说法",
        _ => "",
    }
}

fn register_label(register: &str) -> &'static str {
    match register {
        "casual" => "随意",
        "neutral" => "中性",
        "formal" => "正式",
        _ => "",
    }
}

fn mapping_label(mapping: &str) -> &'static str {
    match mapping {
        "direct" => "直接对应",
        "partial" => "语感不同",
        "none" => "没有对应",
        _ => "",
    }
}

fn mapping_style(mapping: &str) -> Style {
    match mapping {
        "none" => Style::Orange,
        "partial" => Style::Teal,
        _ => Style::Dim,
    }
}

fn fix_label(kind: &str) -> &'static str {
    match kind {
        "spelling" => "拼写",
        "word" => "用词",
        "grammar" => "语法",
        "register" => "语气",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use subtext_coach::{Alternative, ComposeOption, DecodePoint, Fix, Tone};

    use super::*;

    fn decoded() -> Decoded {
        Decoded {
            situation: "同事在催你".to_owned(),
            points: vec![DecodePoint {
                phrase: "lmk".to_owned(),
                kind: "abbr".to_owned(),
                literal: "let me know".to_owned(),
                meaning: "告诉我一声".to_owned(),
                zh: "跟我说一声".to_owned(),
                usage: "lmk if the board arrives".to_owned(),
            }],
            tone: Tone {
                register: "casual".to_owned(),
                subtext: "轻微催促".to_owned(),
                contrast: String::new(),
            },
            translation: "嘿，不着急。".to_owned(),
            question: "他多久要回复？".to_owned(),
        }
    }

    #[test]
    fn translation_is_hidden_until_revealed() {
        let hidden = Doc::decode(&decoded(), false).plain_text();
        assert!(!hidden.contains("嘿，不着急"));
        assert!(hidden.contains("lmk") && hidden.contains("语气 · casual"));
        assert!(
            Doc::decode(&decoded(), true)
                .plain_text()
                .contains("嘿，不着急")
        );
    }

    #[test]
    fn abbreviation_tag_is_orange_and_empty_fields_are_dropped() {
        let doc = Doc::decode(&decoded(), false);
        let tag = doc
            .lines
            .iter()
            .flat_map(|line| &line.runs)
            .find(|run| run.text.contains("缩写"))
            .unwrap();
        assert_eq!(tag.style, Style::Orange);
        assert!(!doc.plain_text().contains("对照"));
    }

    #[test]
    fn compose_numbers_options_and_stars_the_recommended() {
        let composed = Composed {
            context: "发给教授的邮件".to_owned(),
            options: vec![
                ComposeOption {
                    register: "neutral".into(),
                    text: "Can I have until Friday?".into(),
                    recommended: false,
                },
                ComposeOption {
                    register: "formal".into(),
                    text: "Would it be possible to have until Friday?".into(),
                    recommended: true,
                },
            ],
            ..Composed::default()
        };
        let text = Doc::compose(&composed).plain_text();
        assert!(text.contains("1  [中性]") && text.contains("2  ★ [正式]"));
        assert!(text.contains("Would it be possible"));
    }

    #[test]
    fn email_option_blank_lines_are_squeezed() {
        assert_eq!(
            compact("Hi Prof. Lee,\n\nThank you.\n\n\nBest"),
            "Hi Prof. Lee,\nThank you.\nBest"
        );
    }

    #[test]
    fn edit_lists_fixes_and_keeps() {
        let edited = Edited {
            corrected: "Can you check my wiring?".to_owned(),
            fixes: vec![Fix {
                from: "u".into(),
                to: "you".into(),
                kind: "register".into(),
                why: "对教授要写全".into(),
            }],
            kept: vec!["ngl 很自然".to_owned()],
            translation: "你能帮我看看接线吗？".to_owned(),
            alternatives: vec![Alternative {
                text: "Could you take a look at my wiring?".to_owned(),
                why: "更像求助时的口吻".to_owned(),
            }],
            pattern: String::new(),
        };
        let text = Doc::edit(&edited).plain_text();
        assert!(text.contains("u → you") && text.contains("语气") && text.contains("✓ ngl 很自然"));
        assert!(text.contains("意思　你能帮我看看接线吗？"));
        assert!(text.contains("更地道  Could you take a look at my wiring?"));
    }
}
