//! 开发预览：把一份教练回复渲染成 PNG，不装输入法、不要屏幕录制权限就能看面板长什么样。
//!
//! `subtext-macos --coach-preview <decode|compose|edit> <回复.json> <输出.png> [light|dark] [reveal] [replace]`

use objc2::MainThreadMarker;
use objc2_app_kit::{NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication};
use objc2_foundation::NSRect;
use subtext_coach::{CoachOutput, Mode};

use super::action::{CoachAction, MAX_OPTIONS};
use super::doc::Doc;
use super::panel::{CoachPanel, PanelContent};

/// 命令行参数里有 `--coach-preview` 就渲染并返回退出码；没有返回 `None`。
pub fn run_if_requested(mtm: MainThreadMarker) -> Option<i32> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let position = arguments
        .iter()
        .position(|argument| argument == "--coach-preview")?;
    let rest = &arguments[position + 1..];
    let [mode, reply, output, options @ ..] = rest else {
        eprintln!(
            "用法：--coach-preview <decode|compose|edit> <回复.json> <输出.png> [light|dark] [reveal] [replace]"
        );
        return Some(2);
    };
    Some(match render(mtm, mode, reply, output, options) {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("{message}");
            1
        }
    })
}

fn render(
    mtm: MainThreadMarker,
    mode: &str,
    reply: &str,
    output: &str,
    options: &[String],
) -> Result<(), String> {
    let mode = match mode {
        "decode" => Mode::Decode,
        "compose" => Mode::Compose,
        "edit" => Mode::Edit,
        other => return Err(format!("不认识的模式：{other}")),
    };
    let text =
        std::fs::read_to_string(reply).map_err(|error| format!("读不了 {reply}：{error}"))?;
    let parsed = CoachOutput::parse(mode, &text).map_err(|error| format!("解析失败：{error}"))?;
    let reveal = options.iter().any(|option| option == "reveal");
    let replace = options.iter().any(|option| option == "replace");
    let app = NSApplication::sharedApplication(mtm);
    let _ = &app;
    let content = content(&parsed, reveal, replace);
    let mut panel = CoachPanel::new(mtm);
    // SAFETY: 只读 AppKit 导出的外观名常量
    let appearance = unsafe {
        if options.iter().any(|option| option == "dark") {
            NSAppearance::appearanceNamed(NSAppearanceNameDarkAqua)
        } else {
            NSAppearance::appearanceNamed(NSAppearanceNameAqua)
        }
    };
    panel.set_appearance(appearance.as_deref());
    panel.show(&content, NSRect::ZERO);
    let png = panel.snapshot_png().ok_or("渲染失败")?;
    std::fs::write(output, png).map_err(|error| format!("写不了 {output}：{error}"))?;
    println!("已渲染 {output}");
    Ok(())
}

fn content(output: &CoachOutput, reveal: bool, replace: bool) -> PanelContent {
    let close = ("关闭".to_owned(), CoachAction::Close);
    let footer = "本机 Claude Code · Esc 关闭".to_owned();
    match output {
        CoachOutput::Decode(decoded) => {
            let mut buttons = Vec::new();
            if !reveal {
                buttons.push(("显示译文".to_owned(), CoachAction::Reveal));
            }
            buttons.push(close);
            PanelContent {
                doc: Doc::decode(decoded, reveal),
                buttons,
                footer,
            }
        }
        CoachOutput::Compose(composed) => {
            let mut buttons = Vec::new();
            for index in 0..composed.options.len().min(MAX_OPTIONS) {
                let (title, action) = if replace {
                    (format!("替换 ⌥{}", index + 1), CoachAction::Replace(index))
                } else {
                    (format!("复制 {}", index + 1), CoachAction::Copy(index))
                };
                buttons.push((title, action));
            }
            if replace {
                buttons.push(("复制推荐".to_owned(), CoachAction::Copy(0)));
            }
            buttons.push(close);
            PanelContent {
                doc: Doc::compose(composed),
                buttons,
                footer,
            }
        }
        CoachOutput::Edit(edited) => PanelContent {
            doc: Doc::edit(edited),
            buttons: vec![("复制".to_owned(), CoachAction::CopyEdited), close],
            footer,
        },
    }
}
