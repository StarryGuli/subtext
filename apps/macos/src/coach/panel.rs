//! 教练面板：不抢焦点的浮动 NSPanel，内容是一段带样式的文字加几个按钮，高度随内容变。
//!
//! 与候选窗口同级，但可以点按钮、选中文字复制；点按钮不会让面板成为键窗口，应用里的光标不丢。

use objc2::rc::Retained;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{
    NSAppearanceCustomization, NSAttributedStringNSStringDrawingDeprecated, NSBackingStoreType,
    NSBorderType, NSBox, NSBoxType, NSButton, NSColor, NSControlSize, NSFont, NSPanel,
    NSScrollView, NSStringDrawingOptions, NSTextField, NSView, NSWindowCollectionBehavior,
    NSWindowLevel, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use super::action::CoachAction;
use super::attributed;
use super::doc::Doc;
use super::target::CoachTarget;
use crate::candidates::place_near;

/// 与候选窗口、状态条同级。
const POPUP_MENU_LEVEL: NSWindowLevel = 101;

/// 面板宽度。
const WIDTH: f64 = 440.0;

/// 内边距。
const PAD: f64 = 14.0;

/// 按钮行高度与按钮之间的间隙。
const BUTTON_HEIGHT: f64 = 22.0;
const BUTTON_GAP: f64 = 8.0;

/// 正文与按钮、按钮与脚注之间的间隙。
const SECTION_GAP: f64 = 10.0;

/// 正文区最高多少；超过就放进滚动区，面板不会比屏幕还高。
const MAX_TEXT_HEIGHT: f64 = 540.0;

/// 圆角半径。
const CORNER_RADIUS: f64 = 10.0;

/// 面板的内容：文字、按钮与一行脚注。
pub struct PanelContent {
    pub doc: Doc,

    pub buttons: Vec<(String, CoachAction)>,

    /// 脚注：当前后端等弱信息；空则不画。
    pub footer: String,
}

pub struct CoachPanel {
    panel: Retained<NSPanel>,

    frame_box: Retained<NSBox>,

    target: Retained<CoachTarget>,

    mtm: MainThreadMarker,
}

impl CoachPanel {
    pub fn new(mtm: MainThreadMarker) -> Self {
        let frame_box = NSBox::new(mtm);
        frame_box.setBoxType(NSBoxType::Custom);
        frame_box.setTitlePosition(objc2_app_kit::NSTitlePosition::NoTitle);
        frame_box.setBorderWidth(0.5);
        frame_box.setCornerRadius(CORNER_RADIUS);
        frame_box.setContentViewMargins(NSSize::new(0.0, 0.0));
        frame_box.setFillColor(&NSColor::windowBackgroundColor());
        frame_box.setBorderColor(&NSColor::separatorColor());
        Self {
            panel: build_panel(mtm, &frame_box),
            frame_box,
            target: CoachTarget::new(mtm),
            mtm,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.panel.isVisible()
    }

    /// 强制浅色 / 深色外观；预览与截图用。
    pub fn set_appearance(&self, appearance: Option<&objc2_app_kit::NSAppearance>) {
        self.panel.setAppearance(appearance);
    }

    pub fn hide(&self) {
        self.panel.orderOut(None);
    }

    /// 摆内容并显示在 `anchor`（屏幕坐标的矩形）附近。
    pub fn show(&mut self, content: &PanelContent, anchor: NSRect) {
        let size = self.layout(content);
        self.panel
            .setFrame_display(NSRect::new(place_near(self.mtm, size, anchor), size), true);
        self.panel.orderFrontRegardless();
    }

    /// 把内容摆进盒子，返回面板该有的尺寸。
    fn layout(&mut self, content: &PanelContent) -> NSSize {
        let inner = WIDTH - 2.0 * PAD;
        let text = attributed::build(&content.doc);
        let full_text_height = text
            .boundingRectWithSize_options(
                NSSize::new(inner, 10_000.0),
                NSStringDrawingOptions::UsesLineFragmentOrigin
                    | NSStringDrawingOptions::UsesFontLeading,
            )
            .size
            .height
            .ceil()
            + 2.0;

        let scrolls = full_text_height > MAX_TEXT_HEIGHT;
        let text_height = full_text_height.min(MAX_TEXT_HEIGHT);
        let has_buttons = !content.buttons.is_empty();
        let has_footer = !content.footer.is_empty();
        let footer_height = if has_footer { 14.0 } else { 0.0 };
        let mut height = PAD * 2.0 + text_height;
        if has_buttons {
            height += SECTION_GAP + BUTTON_HEIGHT;
        }
        if has_footer {
            height += SECTION_GAP * 0.6 + footer_height;
        }

        let Some(container) = self.frame_box.contentView() else {
            return NSSize::new(WIDTH, height);
        };
        for subview in container.subviews().iter() {
            subview.removeFromSuperview();
        }
        self.frame_box
            .setFrame(NSRect::new(NSPoint::ZERO, NSSize::new(WIDTH, height)));

        let mut y = height - PAD - text_height;
        let label = NSTextField::labelWithAttributedString(&text, self.mtm);
        label.setSelectable(true);
        if scrolls {
            // 内容比上限高：放进滚动区，滚轮可滚，滚动条自动隐藏
            label.setFrame(NSRect::new(
                NSPoint::ZERO,
                NSSize::new(inner, full_text_height),
            ));
            let scroll = NSScrollView::new(self.mtm);
            scroll.setHasVerticalScroller(true);
            scroll.setAutohidesScrollers(true);
            scroll.setDrawsBackground(false);
            scroll.setBorderType(NSBorderType::NoBorder);
            scroll.setFrame(NSRect::new(
                NSPoint::new(PAD, y),
                NSSize::new(inner, text_height),
            ));
            scroll.setDocumentView(Some(&label));
            container.addSubview(&scroll);
        } else {
            label.setFrame(NSRect::new(
                NSPoint::new(PAD, y),
                NSSize::new(inner, text_height),
            ));
            container.addSubview(&label);
        }

        if has_buttons {
            y -= SECTION_GAP + BUTTON_HEIGHT;
            let mut x = PAD;
            for (title, action) in &content.buttons {
                let button = self.button(title, *action);
                let width = button.frame().size.width.max(52.0);
                button.setFrame(NSRect::new(
                    NSPoint::new(x, y),
                    NSSize::new(width, BUTTON_HEIGHT),
                ));
                container.addSubview(&button);
                x += width + BUTTON_GAP;
            }
        }
        if has_footer {
            let footer =
                NSTextField::labelWithString(&NSString::from_str(&content.footer), self.mtm);
            footer.setFont(Some(&NSFont::systemFontOfSize(10.0)));
            footer.setTextColor(Some(&NSColor::tertiaryLabelColor()));
            footer.setFrame(NSRect::new(
                NSPoint::new(PAD, PAD * 0.6),
                NSSize::new(inner, footer_height),
            ));
            container.addSubview(&footer);
        }
        NSSize::new(WIDTH, height)
    }

    fn button(&self, title: &str, action: CoachAction) -> Retained<NSButton> {
        // SAFETY: target 的 `clicked:` 在 CoachTarget 上定义，action 与之同名
        let button = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str(title),
                Some(&self.target),
                Some(sel!(clicked:)),
                self.mtm,
            )
        };
        button.setControlSize(NSControlSize::Small);
        button.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        button.setTag(action.tag());
        button.sizeToFit();
        button
    }
}

/// 建面板：无边框、不抢焦点、透明背景（圆角由盒子画）、带阴影、出现在所有 Space。
fn build_panel(mtm: MainThreadMarker, content: &NSBox) -> Retained<NSPanel> {
    let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
        mtm.alloc::<NSPanel>(),
        NSRect::new(NSPoint::ZERO, NSSize::new(WIDTH, 100.0)),
        NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
        NSBackingStoreType::Buffered,
        false,
    );
    panel.setOpaque(false);
    panel.setBackgroundColor(Some(&NSColor::clearColor()));
    panel.setHasShadow(true);
    panel.setBecomesKeyOnlyIfNeeded(true);
    panel.setHidesOnDeactivate(false);
    panel.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Stationary,
    );
    panel.setLevel(POPUP_MENU_LEVEL);
    let view: &NSView = content;
    panel.setContentView(Some(view));
    panel
}

impl CoachPanel {
    /// 把当前面板内容渲染成 PNG 字节：预览与截图用，不需要屏幕录制权限。
    pub fn snapshot_png(&self) -> Option<Vec<u8>> {
        use objc2_app_kit::NSBitmapImageFileType;
        use objc2_foundation::NSDictionary;
        let view: &NSView = &self.frame_box;
        let bounds = view.bounds();
        let rep = view.bitmapImageRepForCachingDisplayInRect(bounds)?;
        view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
        let data = unsafe {
            rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
        }?;
        Some(data.to_vec())
    }
}
