//! 把 [`Doc`] 排成 `NSAttributedString`：字体与颜色全部取系统语义色，深浅色外观自动跟随。

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{
    NSBackgroundColorAttributeName, NSColor, NSFont, NSFontAttributeName, 
    NSFontWeightMedium, NSFontWeightRegular, NSFontWeightSemibold, NSForegroundColorAttributeName,
    NSMutableParagraphStyle, NSParagraphStyleAttributeName,
};
use objc2_foundation::{NSAttributedString, NSDictionary, NSMutableAttributedString, NSString};

use super::doc::{Doc, Style};

/// 同一段落内行与行之间的额外间距。
const LINE_SPACING: f64 = 2.0;

/// 段与段之间：紧挨着的、以及 `gap` 隔开的。
const TIGHT_SPACING: f64 = 2.0;
const GAP_SPACING: f64 = 9.0;

/// 鼠尾草绿底的透明度：浅色下是淡绿，深色下叠在深底上仍然柔和。
const HIGHLIGHT_ALPHA: f64 = 0.28;

pub fn build(doc: &Doc) -> Retained<NSMutableAttributedString> {
    let result = NSMutableAttributedString::new();
    let last = doc.lines.len().saturating_sub(1);
    for (index, line) in doc.lines.iter().enumerate() {
        let paragraph = NSMutableParagraphStyle::new();
        paragraph.setLineSpacing(LINE_SPACING);
        if index > 0 {
            paragraph.setParagraphSpacingBefore(if line.gap { GAP_SPACING } else { TIGHT_SPACING });
        }
        for run in &line.runs {
            let text = if std::ptr::eq(run, line.runs.last().expect("non-empty line")) && index != last {
                format!("{}\n", run.text)
            } else {
                run.text.clone()
            };
            result.appendAttributedString(&styled(&text, run.style, &paragraph));
        }
    }
    result
}

fn styled(text: &str, style: Style, paragraph: &NSMutableParagraphStyle) -> Retained<NSAttributedString> {
    let font = font_for(style);
    let color = color_for(style);
    let background = background_for(style);
    // SAFETY: 只读 AppKit 导出的属性名常量
    let (keys, objects): (Vec<&NSString>, Vec<&AnyObject>) = unsafe {
        let mut keys = vec![
            NSFontAttributeName,
            NSForegroundColorAttributeName,
            NSParagraphStyleAttributeName,
        ];
        let mut objects: Vec<&AnyObject> = vec![&font, &color, paragraph];
        if let Some(background) = &background {
            keys.push(NSBackgroundColorAttributeName);
            objects.push(background);
        }
        (keys, objects)
    };
    let attributes = NSDictionary::from_slices(&keys, &objects);
    unsafe { NSAttributedString::new_with_attributes(&NSString::from_str(text), &attributes) }
}

fn font_for(style: Style) -> Retained<NSFont> {
    // SAFETY: 只读 AppKit 导出的字重常量
    unsafe {
        match style {
            Style::Title => NSFont::systemFontOfSize_weight(11.0, NSFontWeightSemibold),
            Style::Body | Style::Highlight => NSFont::systemFontOfSize_weight(13.0, NSFontWeightRegular),
            Style::Phrase => NSFont::systemFontOfSize_weight(13.5, NSFontWeightMedium),
            Style::Dim | Style::Orange | Style::Teal | Style::Warn => {
                NSFont::systemFontOfSize_weight(12.0, NSFontWeightRegular)
            }
        }
    }
}

fn color_for(style: Style) -> Retained<NSColor> {
    match style {
        Style::Title | Style::Dim => NSColor::secondaryLabelColor(),
        Style::Body | Style::Phrase | Style::Highlight => NSColor::labelColor(),
        Style::Orange => NSColor::systemOrangeColor(),
        Style::Teal => NSColor::systemTealColor(),
        Style::Warn => NSColor::systemRedColor(),
    }
}

fn background_for(style: Style) -> Option<Retained<NSColor>> {
    (style == Style::Highlight)
        .then(|| NSColor::colorWithSRGBRed_green_blue_alpha(176.0 / 255.0, 206.0 / 255.0, 125.0 / 255.0, HIGHLIGHT_ALPHA))
}
