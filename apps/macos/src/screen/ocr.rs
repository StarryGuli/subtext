//! OCR：调随包的 `subtext-ocr`（Swift，系统自带的 Vision，全程本机识别）。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Deserialize;

use super::geometry::NormalizedLine;

/// 帮手程序在 `.app` 的 Resources 里，开发时可以用 `SUBTEXT_OCR_BIN` 指到别处。
pub fn helper_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SUBTEXT_OCR_BIN") {
        return Some(PathBuf::from(path));
    }
    let resources = objc2_foundation::NSBundle::mainBundle().resourcePath()?;
    let path = PathBuf::from(resources.to_string()).join("subtext-ocr");
    path.is_file().then_some(path)
}

#[derive(Deserialize)]
struct Output {
    lines: Vec<Line>,
}

#[derive(Deserialize)]
struct Line {
    text: String,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

/// 识别一张图，返回每行文字与它的比例坐标。
pub fn recognize(helper: &Path, image: &Path) -> Result<Vec<NormalizedLine>, String> {
    let output = Command::new(helper)
        .arg(image)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("起不了 OCR 程序：{error}"))?;
    if !output.status.success() {
        return Err(format!(
            "OCR 失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    parse(&output.stdout)
}

fn parse(json: &[u8]) -> Result<Vec<NormalizedLine>, String> {
    let parsed: Output =
        serde_json::from_slice(json).map_err(|error| format!("OCR 输出不是 JSON：{error}"))?;
    Ok(parsed
        .lines
        .into_iter()
        .map(|line| NormalizedLine {
            text: line.text,
            x: line.x,
            y: line.y,
            width: line.w,
            height: line.h,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_helpers_json() {
        let json = br#"{"width": 880, "height": 1098, "lines": [
            {"text": "hello world", "x": 0.036, "y": 0.027, "w": 0.75, "h": 0.031, "confidence": 0.9}]}"#;
        let lines = parse(json).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "hello world");
        assert!((lines[0].width - 0.75).abs() < 1e-6);
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(parse(b"not json").is_err());
    }
}
