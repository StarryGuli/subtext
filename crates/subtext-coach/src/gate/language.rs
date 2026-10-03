//! 粗粒度的语言判断：够区分「英文消息」「中文句子」「代码 / 链接 / 杂物」即可，不求精确。

/// 汉字（含扩展 A 与兼容区）个数。
pub fn cjk_chars(text: &str) -> usize {
    text.chars().filter(|c| is_cjk(*c)).count()
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xF900..=0xFAFF)
}

/// 英文单词个数：至少含两个字母、由字母与撇号连字符组成的片段。
pub fn english_words(text: &str) -> usize {
    text.split(|c: char| !(c.is_ascii_alphabetic() || c == '\'' || c == '’' || c == '-'))
        .filter(|word| word.chars().filter(|c| c.is_ascii_alphabetic()).count() >= 2)
        .count()
}

/// 主要是英文：字母里拉丁字母占绝大多数，且不是一堆符号。
pub fn is_mostly_english(text: &str) -> bool {
    let latin = text.chars().filter(char::is_ascii_alphabetic).count();
    let cjk = cjk_chars(text);
    if latin == 0 || cjk * 3 > latin {
        return false;
    }
    let visible = text.chars().filter(|c| !c.is_whitespace()).count();
    latin * 10 >= visible * 6
}

/// 看起来是链接、路径、代码或日志而不是一句人话。
pub fn looks_like_machine_text(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return trimmed.split_whitespace().count() == 1;
    }
    if trimmed.starts_with('/') || trimmed.starts_with("~/") || trimmed.starts_with("./") {
        return !trimmed.contains(' ');
    }
    let symbols = trimmed
        .chars()
        .filter(|c| matches!(c, '{' | '}' | ';' | '=' | '<' | '>' | '[' | ']' | '\\' | '|' | '$'))
        .count();
    let visible = trimmed.chars().filter(|c| !c.is_whitespace()).count().max(1);
    symbols * 12 > visible
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_cjk_and_words() {
        assert_eq!(cjk_chars("我想去 the lab"), 3);
        assert_eq!(english_words("don't worry, it's a no-brainer"), 4);
        assert_eq!(english_words("1 2 3"), 0);
    }

    #[test]
    fn mostly_english_tolerates_a_little_chinese() {
        assert!(is_mostly_english("lmk if you want me to just merge it"));
        assert!(is_mostly_english("Thanks 老师 for the feedback on my draft this week"));
        assert!(!is_mostly_english("今天下午三点开会 ok"));
        assert!(!is_mostly_english("12345 67890"));
    }

    #[test]
    fn machine_text_is_recognized() {
        assert!(looks_like_machine_text("https://github.com/a/b"));
        assert!(looks_like_machine_text("/usr/local/bin/cargo"));
        assert!(looks_like_machine_text("fn main() { let x = [1, 2]; x[0] = 3; }"));
        assert!(!looks_like_machine_text("hey, did you get a chance to look at that PR?"));
        assert!(!looks_like_machine_text("see https://example.com for details when you can"));
    }
}
