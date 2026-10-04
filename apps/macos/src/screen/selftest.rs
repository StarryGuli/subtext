//! 屏幕阅读的端到端自测：`subtext-macos --screen-selftest <图片>`。
//!
//! 不截屏、不联网：把给定图片当作截屏结果，走真实的 OCR、块合并、记忆、外发闸门、后端线程，
//! 后端用一个假的 `claude` 脚本（对请求里每个编号回一条译文）。用来在没有屏幕录制权限、没登录命令行的机器上
//! 验证「识别出的块 ↔ 译文」对得上。

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use subtext_coach::CoachConfig;
use subtext_coach::screen::Rect;

use super::reader::Active;
use super::{ocr, target::Target};

/// 假 claude：读 stdin，对每个「N. 」开头的编号行回一条 `译N`，按 stream-json 的格式。
const FAKE_CLAUDE: &str = r#"#!/bin/sh
input=$(cat)
items=$(printf '%s\n' "$input" | awk '/^[0-9]+\. /{n=$1; sub(/\./,"",n); printf "%s{\\\"i\\\":%s,\\\"translation\\\":\\\"译%s\\\",\\\"note\\\":\\\"提示%s\\\"}", (c++?",":""), n, n, n}')
printf '{"type":"result","subtype":"success","is_error":false,"result":"{\\"items\\":[%s]}"}\n' "$items"
"#;

pub fn run_if_requested() -> Option<i32> {
    let arguments: Vec<String> = std::env::args().collect();
    let position = arguments
        .iter()
        .position(|argument| argument == "--screen-selftest")?;
    let Some(image) = arguments.get(position + 1) else {
        eprintln!("用法：--screen-selftest <图片.png>");
        return Some(2);
    };
    Some(execute(&PathBuf::from(image)))
}

fn execute(image: &std::path::Path) -> i32 {
    let Some(helper) = ocr::helper_path() else {
        eprintln!("找不到 OCR 程序：设 SUBTEXT_OCR_BIN 指到 subtext-ocr");
        return 1;
    };
    let script = std::env::temp_dir().join(format!("subtext-fake-claude-{}", std::process::id()));
    if std::fs::write(&script, FAKE_CLAUDE).is_err()
        || std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).is_err()
    {
        eprintln!("写不了假 claude");
        return 1;
    }
    let config = CoachConfig {
        enabled: true,
        claude_path: script.to_string_lossy().into_owned(),
        timeout_ms: 20_000,
        screen_interval_ms: 500,
        ..CoachConfig::default()
    };
    let target = Target::Region {
        bounds: Rect::new(0.0, 0.0, 440.0, 560.0),
    };
    let mut active = Active::new(target, helper, &config);
    active.set_fake_image(image.to_path_buf());

    let started = Instant::now();
    let mut done = false;
    while started.elapsed() < Duration::from_secs(25) {
        active.drive_once();
        let blocks = active.snapshot();
        let english = blocks
            .iter()
            .filter(|b| b.text.is_ascii() || b.text.contains(|c: char| c.is_ascii_alphabetic()))
            .count();
        if !blocks.is_empty()
            && blocks.iter().any(|b| b.analysis.is_some())
            && blocks.iter().all(|b| !b.pending)
            && english > 0
        {
            done = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = std::fs::remove_file(&script);
    println!("识别出 {} 块：", active.snapshot().len());
    for block in active.snapshot() {
        let text: String = block.text.chars().take(48).collect();
        match &block.analysis {
            Some(analysis) => println!(
                "  ✓ {text}  →  {} / {}",
                analysis.translation, analysis.note
            ),
            None => println!("  · {text}"),
        }
    }
    if done { 0 } else { 1 }
}
