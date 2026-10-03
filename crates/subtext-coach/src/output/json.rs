//! 从模型回复里取出 JSON 对象：容忍代码围栏与前后多余的话。

/// 取第一个括号配平的 `{ ... }`，按字符串与转义规则跳过引号里的花括号。
pub fn extract_object(reply: &str) -> Option<&str> {
    let start = reply.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, c) in reply[start..].char_indices() {
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
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&reply[start..start + offset + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_object() {
        assert_eq!(extract_object(r#"{"a": 1}"#), Some(r#"{"a": 1}"#));
    }

    #[test]
    fn fenced_and_chatty_replies() {
        let reply = "好的，结果如下：\n```json\n{\"a\": {\"b\": 2}}\n```\n希望有帮助";
        assert_eq!(extract_object(reply), Some(r#"{"a": {"b": 2}}"#));
    }

    #[test]
    fn braces_inside_strings_do_not_confuse_it() {
        let reply = r#"{"text": "use {x} and \"}\" here", "n": 1} trailing }"#;
        assert_eq!(
            extract_object(reply),
            Some(r#"{"text": "use {x} and \"}\" here", "n": 1}"#)
        );
    }

    #[test]
    fn unbalanced_or_missing_is_none() {
        assert_eq!(extract_object("no json"), None);
        assert_eq!(extract_object(r#"{"a": 1"#), None);
    }
}
