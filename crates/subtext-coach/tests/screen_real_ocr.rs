//! 用真实的 OCR 输出（对本项目的面板截图跑 `subtext-ocr` 得到）验证块合并：
//! 合成数据能过的启发式，不一定能过真实的行框。

use serde::Deserialize;
use subtext_coach::screen::{OcrLine, Rect, group_blocks};

#[derive(Deserialize)]
struct Output {
    width: f32,
    height: f32,
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

/// 读固定样本，换算成「点」：图片宽 880 像素对应 440 点。
fn lines(path: &str) -> Vec<OcrLine> {
    let json = std::fs::read_to_string(path).unwrap();
    let output: Output = serde_json::from_str(&json).unwrap();
    let (width, height) = (output.width / 2.0, output.height / 2.0);
    output
        .lines
        .into_iter()
        .map(|line| OcrLine {
            text: line.text,
            rect: Rect::new(
                line.x * width,
                line.y * height,
                line.w * width,
                line.h * height,
            ),
        })
        .collect()
}

#[test]
fn the_wrapped_example_sentence_is_one_block_and_sections_stay_apart() {
    let lines = lines("tests/fixtures/ocr-decode-light.json");
    let blocks = group_blocks(&lines);
    for block in &blocks {
        println!(
            "[{} 行] {}",
            block.lines,
            block.text.chars().take(70).collect::<String>()
        );
    }
    // 「你可以说：No rush at all, but could you look at my draft by / Friday？」折成两行，必须并成一块
    assert!(
        blocks
            .iter()
            .any(|block| block.text.contains("draft by") && block.text.contains("Friday")),
        "折行的句子没有合到一起"
    );
    // 不同条目（lmk、PR、no rush…）不能被并成一整块
    assert!(
        blocks.len() >= 8,
        "块太少，条目被并到一起了: {}",
        blocks.len()
    );
    assert!(
        blocks.iter().all(|block| block.text.chars().count() < 260),
        "有块长得不像一条消息"
    );
}

#[test]
fn compose_panel_email_paragraphs_do_not_swallow_the_explanations() {
    let blocks = group_blocks(&lines("tests/fixtures/ocr-compose-light.json"));
    for block in &blocks {
        println!(
            "[{} 行] {}",
            block.lines,
            block.text.chars().take(70).collect::<String>()
        );
    }
    assert!(blocks.len() >= 6);
    // 英文邮件正文与下面的中文教学解释不能在同一块里
    assert!(
        !blocks
            .iter()
            .any(|block| { block.text.contains("Hi Prof. Lee") && block.text.contains("直译") })
    );
    // 一封邮件的几行折行要在同一块里
    assert!(blocks.iter().any(|block| {
        block.text.contains("won't be able to finish")
            && block.text.contains("Thank you for your understanding")
    }));
}
