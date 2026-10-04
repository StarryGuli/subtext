//! 流式输出时模型的 JSON 只写了一半：补全括号与引号，取出此刻已经能看的部分。

use serde_json::Value;

/// 最多往回退这么多个切点去找一个能解析的前缀，防止极端输入拖慢。
const MAX_CUTS: usize = 24;

/// 把被截断的 JSON 对象文本解析成 [`Value`]：字符串、数组、对象没写完就当场补上，
/// 补完仍然不合法（停在 `"key":` 或写了一半的数字之类）就往回退到上一个逗号或括号再试。
/// 还没出现 `{` 或一个字段都没写完时返回 `None`。
pub fn snapshot(prefix: &str) -> Option<Value> {
    let start = prefix.find('{')?;
    let text = &prefix[start..];
    let mut attempt = text.len();
    for _ in 0..MAX_CUTS {
        let candidate = &text[..attempt];
        if let Ok(value) = serde_json::from_str::<Value>(&close_open(candidate)) {
            return Some(value);
        }
        // 往回退到上一个逗号、`{` 或 `[`（保留括号本身，去掉后面写了一半的东西）
        let cut = candidate[..candidate.len().saturating_sub(1)].rfind([',', '{', '['])?;
        attempt = if candidate.as_bytes()[cut] == b',' {
            cut
        } else {
            cut + 1
        };
    }
    None
}

/// 补上没关的字符串、数组与对象；末尾悬着的逗号去掉。
fn close_open(text: &str) -> String {
    let mut stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    for c in text.chars() {
        if in_string {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => stack.push('}'),
            '[' => stack.push(']'),
            '}' | ']' => {
                stack.pop();
            }
            _ => {}
        }
    }
    let mut out = text.to_owned();
    if in_string {
        // 停在转义符中间（`\` 后面还没来字符）：丢掉那个反斜杠
        if escaped {
            out.pop();
        }
        out.push('"');
    }
    let trimmed = out.trim_end().trim_end_matches(',').to_owned();
    let mut out = trimmed;
    while let Some(closer) = stack.pop() {
        out.push(closer);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_json_is_returned_as_is() {
        let value = snapshot(r#"{"a": [1, 2], "b": "x"}"#).unwrap();
        assert_eq!(value["b"], "x");
    }

    #[test]
    fn unfinished_string_is_closed() {
        let value = snapshot(r#"{"situation": "同事在催你，但催得很"#).unwrap();
        assert_eq!(value["situation"], "同事在催你，但催得很");
    }

    #[test]
    fn dangling_key_and_comma_are_dropped() {
        let value = snapshot(r#"{"situation": "ok", "points": [{"phrase": "lmk"}, "#).unwrap();
        assert_eq!(value["situation"], "ok");
        assert_eq!(value["points"][0]["phrase"], "lmk");
        let value = snapshot(r#"{"situation": "ok", "tone":"#).unwrap();
        assert_eq!(value["situation"], "ok");
        assert!(value.get("tone").is_none());
    }

    #[test]
    fn escape_cut_in_the_middle_is_safe() {
        let value = snapshot("{\"text\": \"say \\").unwrap();
        assert_eq!(value["text"], "say ");
    }

    #[test]
    fn leading_chatter_and_fences_are_skipped() {
        let value = snapshot("好的：\n```json\n{\"a\": \"b").unwrap();
        assert_eq!(value["a"], "b");
    }

    #[test]
    fn nothing_yet_is_none() {
        assert!(snapshot("").is_none());
        assert!(snapshot("正在思考").is_none());
    }

    #[test]
    fn every_prefix_of_a_real_reply_is_either_none_or_valid() {
        let full = r#"{"context": "发给教授", "options": [{"register": "formal", "text": "Hi \"Prof\",\nok", "recommended": true}], "points": []}"#;
        for end in (0..=full.len()).filter(|i| full.is_char_boundary(*i)) {
            if let Some(value) = snapshot(&full[..end]) {
                assert!(value.is_object());
            }
        }
        assert_eq!(snapshot(full).unwrap()["options"][0]["recommended"], true);
    }
}
