//! 教练输出：三种模式各一个结构，从模型回复里容错解析出来。

mod alternative;
mod composed;
mod decoded;
mod edited;
mod json;
mod partial;
mod screened;

pub use alternative::Alternative;
pub use composed::{ComposeOption, ComposePoint, Composed};
pub use decoded::{DecodePoint, Decoded, Tone};
pub use edited::{Edited, Fix};
pub use screened::{ScreenItem, Screened};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{CoachError, Mode};

/// 一次教练的结果。可以序列化：结果缓存落盘用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CoachOutput {
    Decode(Decoded),
    Compose(Composed),
    Edit(Edited),
    Screen(Screened),

    /// 模型没按 JSON 格式回，而是直接说了一段话：原样显示，总比一句「没有可用的回复」强。
    Plain {
        mode: Mode,
        text: String,
    },
}

impl CoachOutput {
    /// 解析模型回复。空结果当作失败，免得面板弹出一张空卡片。
    ///
    /// 屏幕阅读之外的模式，回复不是约定的 JSON（模型直接说了段话、字段类型不对）时退成 [`Self::Plain`]：
    /// 原文显示给用户。屏幕阅读的结果要按编号对到各条消息，不是 JSON 就没法用，仍然报错。
    pub fn parse(mode: Mode, reply: &str) -> Result<Self, CoachError> {
        match Self::parse_structured(mode, reply) {
            Err(CoachError::BadReply(_)) if mode != Mode::Screen && !reply.trim().is_empty() => {
                Ok(Self::Plain {
                    mode,
                    text: plain_text(reply),
                })
            }
            other => other,
        }
    }

    fn parse_structured(mode: Mode, reply: &str) -> Result<Self, CoachError> {
        let output = match mode {
            Mode::Decode => Self::Decode(parse_object(reply)?),
            Mode::Compose => Self::Compose(parse_object(reply)?),
            Mode::Edit => Self::Edit(parse_object(reply)?),
            Mode::Screen => Self::Screen(parse_object(reply)?),
        };
        if output.is_empty() {
            return Err(CoachError::EmptyReply);
        }
        Ok(output)
    }

    /// 流式输出写到一半时，取此刻能看的部分；一点内容都还没有返回 `None`。
    pub fn parse_partial(mode: Mode, text: &str) -> Option<Self> {
        let Some(value) = partial::snapshot(text) else {
            // 还没出现 `{` 就已经写了一大段：多半是纯文本回复，边写边显示
            let plain = plain_text(text);
            return (mode != Mode::Screen && !text.contains('{') && plain.chars().count() >= 8)
                .then_some(Self::Plain { mode, text: plain });
        };
        let output = match mode {
            Mode::Decode => Self::Decode(serde_json::from_value(value).ok()?),
            Mode::Compose => Self::Compose(serde_json::from_value(value).ok()?),
            Mode::Edit => Self::Edit(serde_json::from_value(value).ok()?),
            Mode::Screen => Self::Screen(serde_json::from_value(value).ok()?),
        };
        (!output.is_empty()).then_some(output)
    }

    pub fn mode(&self) -> Mode {
        match self {
            Self::Decode(_) => Mode::Decode,
            Self::Compose(_) => Mode::Compose,
            Self::Edit(_) => Mode::Edit,
            Self::Screen(_) => Mode::Screen,
            Self::Plain { mode, .. } => *mode,
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::Decode(output) => output.is_empty(),
            Self::Compose(output) => output.is_empty(),
            Self::Edit(output) => output.is_empty(),
            Self::Screen(output) => output.is_empty(),
            Self::Plain { text, .. } => text.trim().is_empty(),
        }
    }
}

fn parse_object<T: DeserializeOwned>(reply: &str) -> Result<T, CoachError> {
    let object = json::extract_object(reply).ok_or_else(|| CoachError::BadReply(preview(reply)))?;
    serde_json::from_str(object).map_err(|error| CoachError::BadReply(error.to_string()))
}

/// 纯文本回复：去掉首尾的代码围栏标记，其余原样。
fn plain_text(reply: &str) -> String {
    reply
        .lines()
        .filter(|line| !line.trim_start().starts_with("```"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

/// 错误信息里只留回复的开头，别把整段塞进日志。
fn preview(reply: &str) -> String {
    reply.chars().take(120).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_reply_with_missing_fields_still_parses() {
        let reply = r#"```json
{"situation": "同事在催你", "points": [{"phrase": "lmk", "meaning": "let me know"}]}
```"#;
        let CoachOutput::Decode(decoded) = CoachOutput::parse(Mode::Decode, reply).unwrap() else {
            panic!("expected decode");
        };
        assert_eq!(decoded.situation, "同事在催你");
        assert_eq!(decoded.points[0].phrase, "lmk");
        assert!(decoded.tone.register.is_empty());
    }

    #[test]
    fn compose_reply_parses_options() {
        let reply = r#"{"options": [{"register": "formal", "text": "Would it be possible to have until Friday?", "recommended": true}]}"#;
        let CoachOutput::Compose(composed) = CoachOutput::parse(Mode::Compose, reply).unwrap()
        else {
            panic!("expected compose");
        };
        assert!(composed.recommended().unwrap().text.starts_with("Would"));
    }

    #[test]
    fn empty_replies_are_errors_but_prose_is_shown_as_plain_text() {
        assert!(matches!(
            CoachOutput::parse(Mode::Decode, "{}"),
            Err(CoachError::EmptyReply)
        ));
        assert!(matches!(
            CoachOutput::parse(Mode::Edit, "  \n "),
            Err(CoachError::BadReply(_))
        ));
        // 模型没按格式回、直接说了段话：原样显示
        let CoachOutput::Plain { mode, text } =
            CoachOutput::parse(Mode::Edit, "```\n这句话没有错，不用改。\n```").unwrap()
        else {
            panic!("expected plain");
        };
        assert_eq!(mode, Mode::Edit);
        assert_eq!(text, "这句话没有错，不用改。");
        // JSON 字段类型不对也不丢：显示原文
        assert!(matches!(
            CoachOutput::parse(Mode::Compose, r#"{"options": "not a list"}"#),
            Ok(CoachOutput::Plain { .. })
        ));
    }

    #[test]
    fn screen_replies_must_stay_structured() {
        assert!(matches!(
            CoachOutput::parse(Mode::Screen, "抱歉，我没法处理这些"),
            Err(CoachError::BadReply(_))
        ));
        assert!(
            CoachOutput::parse_partial(Mode::Screen, "抱歉，我没法处理这些消息，因为").is_none()
        );
    }

    #[test]
    fn plain_text_streams_before_any_brace_appears() {
        assert!(CoachOutput::parse_partial(Mode::Decode, "这条消息").is_none());
        let partial =
            CoachOutput::parse_partial(Mode::Decode, "这条消息的意思是对方在客气地催你").unwrap();
        assert!(matches!(partial, CoachOutput::Plain { .. }));
        // 一出现 { 就按 JSON 走
        assert!(matches!(
            CoachOutput::parse_partial(Mode::Decode, r#"{"situation": "同事在催你"#),
            Some(CoachOutput::Decode(_))
        ));
    }

    #[test]
    fn outputs_round_trip_through_json_for_the_disk_cache() {
        let output = CoachOutput::Compose(Composed {
            context: "发给教授".to_owned(),
            ..Composed::default()
        });
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"kind\":\"compose\""));
        assert_eq!(serde_json::from_str::<CoachOutput>(&json).unwrap(), output);
        let plain = CoachOutput::Plain {
            mode: Mode::Edit,
            text: "ok".to_owned(),
        };
        assert_eq!(
            serde_json::from_str::<CoachOutput>(&serde_json::to_string(&plain).unwrap()).unwrap(),
            plain
        );
    }
}
