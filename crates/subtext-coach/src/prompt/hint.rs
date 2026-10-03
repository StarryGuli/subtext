//! 由当前应用推断场合与语域倾向。只是线索，用户的文字与对方的语气优先。

/// (应用名里的关键词, 倾向)。按顺序匹配，小写比较。
const HINTS: &[(&[&str], &str)] = &[
    (
        &["mail", "outlook", "spark", "airmail", "superhuman"],
        "邮件，默认偏正式，先说事再给原因",
    ),
    (
        &[
            "messages",
            "imessage",
            "whatsapp",
            "telegram",
            "discord",
            "slack",
            "wechat",
            "xinwechat",
            "signal",
            "line",
            "teams",
        ],
        "即时聊天，默认偏随意，可以用常见缩写",
    ),
    (
        &[
            "xcode",
            "vscode",
            "visualstudio",
            "zed",
            "jetbrains",
            "cursor",
            "terminal",
            "iterm",
            "warp",
            "ghostty",
        ],
        "开发场合：技术圈说话随意但要精确，GitHub issue / PR 讨论用中性偏随意",
    ),
    (
        &["safari", "chrome", "firefox", "arc", "edge", "brave"],
        "浏览器，场合不明，按内容与对方语气判断",
    ),
];

/// 取应用的语域倾向；认不得就返回 `None`。
pub fn register_hint(app: &str) -> Option<&'static str> {
    let app = app.to_lowercase();
    HINTS
        .iter()
        .find(|(keywords, _)| keywords.iter().any(|keyword| app.contains(keyword)))
        .map(|(_, hint)| *hint)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_apps_map_to_hints() {
        assert!(register_hint("com.apple.mail").unwrap().contains("邮件"));
        assert!(register_hint("com.hnc.Discord").unwrap().contains("聊天"));
        assert!(
            register_hint("com.microsoft.VSCode")
                .unwrap()
                .contains("开发")
        );
        assert_eq!(register_hint("com.example.unknown"), None);
    }
}
