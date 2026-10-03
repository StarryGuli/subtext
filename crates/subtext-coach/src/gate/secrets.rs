//! 密钥与口令的特征：命中就绝不外发。宁可多拦，漏拦的代价更大。

/// 明显的凭据前缀与关键词（小写比较）。
const MARKERS: &[&str] = &[
    "-----begin",
    "bearer ",
    "authorization:",
    "password",
    "passwd",
    "passcode",
    "api_key",
    "apikey",
    "api-key",
    "secret",
    "private key",
    "token=",
    "token:",
    "密码",
    "口令",
    "密钥",
    "验证码",
];

/// 常见服务的密钥前缀，后面跟一串字母数字。
const KEY_PREFIXES: &[&str] = &["sk-", "ghp_", "gho_", "ghs_", "github_pat_", "xoxb-", "xoxp-", "akia", "aiza"];

/// 文本里有没有疑似凭据。
pub fn contains_secret(text: &str) -> bool {
    let lowered = text.to_lowercase();
    if MARKERS.iter().any(|marker| lowered.contains(marker)) {
        return true;
    }
    lowered
        .split(|c: char| c.is_whitespace() || matches!(c, '=' | ':' | ',' | ';' | '"' | '\'' | '(' | ')'))
        .any(|word| has_key_prefix(word) || looks_like_token(word) || looks_like_card_number(word))
}

fn has_key_prefix(word: &str) -> bool {
    let word = word.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    KEY_PREFIXES
        .iter()
        .any(|prefix| word.starts_with(prefix) && word.len() >= prefix.len() + 12)
}

/// 很长、没有空格、字母数字混杂的一串：多半是令牌或哈希。
fn looks_like_token(word: &str) -> bool {
    let word = word.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    word.len() >= 32
        && word.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '/' | '='))
        && word.chars().any(|c| c.is_ascii_digit())
        && word.chars().any(|c| c.is_ascii_alphabetic())
}

/// 13 到 19 位连写的数字（可带连字符）。
fn looks_like_card_number(word: &str) -> bool {
    let digits: String = word.chars().filter(|c| *c != '-').collect();
    (13..=19).contains(&digits.len()) && digits.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_credentials() {
        assert!(contains_secret("my password is hunter2"));
        assert!(contains_secret("export KEY=sk-abcdefghijklmnopqrstuv"));
        assert!(contains_secret("token: eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9abcdef1234"));
        assert!(contains_secret("卡号 4111111111111111"));
        assert!(contains_secret("-----BEGIN RSA PRIVATE KEY-----"));
        assert!(contains_secret("你的验证码是 123456"));
    }

    #[test]
    fn leaves_ordinary_messages_alone() {
        assert!(!contains_secret("hey, no rush at all but did you look at that PR?"));
        assert!(!contains_secret("Our meeting is on 2026-10-12 at 3pm"));
        assert!(!contains_secret("I'll send the report by Friday"));
    }
}
