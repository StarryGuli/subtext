//! 教练输出：三种模式各一个结构，从模型回复里容错解析出来。

mod alternative;
mod composed;
mod decoded;
mod edited;
mod json;

pub use alternative::Alternative;
pub use composed::{ComposeOption, ComposePoint, Composed};
pub use decoded::{DecodePoint, Decoded, Tone};
pub use edited::{Edited, Fix};

use serde::de::DeserializeOwned;

use crate::{CoachError, Mode};

/// 一次教练的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoachOutput {
    Decode(Decoded),
    Compose(Composed),
    Edit(Edited),
}

impl CoachOutput {
    /// 解析模型回复。空结果当作失败，免得面板弹出一张空卡片。
    pub fn parse(mode: Mode, reply: &str) -> Result<Self, CoachError> {
        let output = match mode {
            Mode::Decode => Self::Decode(parse_object(reply)?),
            Mode::Compose => Self::Compose(parse_object(reply)?),
            Mode::Edit => Self::Edit(parse_object(reply)?),
        };
        if output.is_empty() {
            return Err(CoachError::EmptyReply);
        }
        Ok(output)
    }

    pub fn mode(&self) -> Mode {
        match self {
            Self::Decode(_) => Mode::Decode,
            Self::Compose(_) => Mode::Compose,
            Self::Edit(_) => Mode::Edit,
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::Decode(output) => output.is_empty(),
            Self::Compose(output) => output.is_empty(),
            Self::Edit(output) => output.is_empty(),
        }
    }
}

fn parse_object<T: DeserializeOwned>(reply: &str) -> Result<T, CoachError> {
    let object = json::extract_object(reply).ok_or_else(|| CoachError::BadReply(preview(reply)))?;
    serde_json::from_str(object).map_err(|error| CoachError::BadReply(error.to_string()))
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
    fn empty_or_garbage_replies_are_errors() {
        assert!(matches!(
            CoachOutput::parse(Mode::Decode, "{}"),
            Err(CoachError::EmptyReply)
        ));
        assert!(matches!(
            CoachOutput::parse(Mode::Edit, "sorry, I can't"),
            Err(CoachError::BadReply(_))
        ));
        assert!(matches!(
            CoachOutput::parse(Mode::Compose, r#"{"options": "not a list"}"#),
            Err(CoachError::BadReply(_))
        ));
    }
}
