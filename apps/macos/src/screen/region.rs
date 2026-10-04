//! 框选区域：每块屏幕盖一层半透明的选框层，拖出一个矩形就是要读的区域。单击不拖（或太小）取消。
//!
//! 选框层是不抢焦点的面板，鼠标事件照样收得到；选完交给 [`crate::host::screen_region_done`]，
//! 由屏幕阅读在下一次轮询里关掉选框层并开始读（不在鼠标事件处理的半途销毁收事件的视图）。

use std::cell::Cell;

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSBackingStoreType, NSBezierPath, NSColor, NSCompositingOperation, NSCursor, NSEvent, NSPanel,
    NSRectFillUsingOperation, NSScreen, NSView, NSWindowCollectionBehavior, NSWindowLevel,
    NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use subtext_coach::screen::Rect;

use super::geometry;

/// 比任何普通窗口、菜单都高。
const OVERLAY_LEVEL: NSWindowLevel = 1000;

/// 拖出的框小于这个边长（点）就当作单击、取消。
const MIN_SIDE: f64 = 40.0;

struct SelectionState {
    start: Cell<Option<NSPoint>>,

    current: Cell<NSPoint>,
}

define_class!(
    // SAFETY: NSView 允许子类化；没有实现 Drop。
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = SelectionState]
    struct SelectionView;

    impl SelectionView {
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let point = self.convertPoint_fromView(event.locationInWindow(), None);
            self.ivars().start.set(Some(point));
            self.ivars().current.set(point);
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            let point = self.convertPoint_fromView(event.locationInWindow(), None);
            self.ivars().current.set(point);
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            let point = self.convertPoint_fromView(event.locationInWindow(), None);
            self.ivars().current.set(point);
            let chosen = self.selection().filter(|rect| {
                rect.size.width >= MIN_SIDE && rect.size.height >= MIN_SIDE
            });
            // 视图在窗口里的位置 → 屏幕坐标（AppKit，原点在主屏左下）→ CG 坐标（原点在主屏左上）
            let region = chosen.and_then(|rect| {
                let window = self.window()?;
                let origin = window.frame().origin;
                let primary = primary_height(self.mtm())?;
                let cocoa_x = origin.x + rect.origin.x;
                let cocoa_y = origin.y + rect.origin.y;
                Some(Rect::new(
                    cocoa_x as f32,
                    geometry::cg_to_cocoa_y(cocoa_y as f32, rect.size.height as f32, primary) as f32,
                    rect.size.width as f32,
                    rect.size.height as f32,
                ))
            });
            crate::host::screen_region_done(region);
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            NSColor::colorWithCalibratedWhite_alpha(0.0, 0.30).setFill();
            NSBezierPath::fillRect(bounds);
            if let Some(selection) = self.selection() {
                // 选中的区域挖空，露出下面真实的屏幕；再描一圈边
                NSRectFillUsingOperation(selection, NSCompositingOperation::Clear);
                NSColor::systemGreenColor().setStroke();
                let outline = NSBezierPath::bezierPathWithRect(selection);
                outline.setLineWidth(2.0);
                outline.stroke();
            }
        }
    }
);

impl SelectionView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(SelectionState {
            start: Cell::new(None),
            current: Cell::new(NSPoint::ZERO),
        });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    /// 拖出的矩形（视图坐标）；还没按下就没有。
    fn selection(&self) -> Option<NSRect> {
        let start = self.ivars().start.get()?;
        let current = self.ivars().current.get();
        Some(NSRect::new(
            NSPoint::new(start.x.min(current.x), start.y.min(current.y)),
            NSSize::new((start.x - current.x).abs(), (start.y - current.y).abs()),
        ))
    }
}

/// 盖在所有屏幕上的选框层；丢掉它就收起。
pub struct RegionPicker {
    panels: Vec<Retained<NSPanel>>,
}

impl RegionPicker {
    pub fn begin(mtm: MainThreadMarker) -> Self {
        let panels = NSScreen::screens(mtm)
            .iter()
            .map(|screen| {
                let frame = screen.frame();
                let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
                    mtm.alloc::<NSPanel>(),
                    frame,
                    NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
                    NSBackingStoreType::Buffered,
                    false,
                );
                panel.setOpaque(false);
                panel.setBackgroundColor(Some(&NSColor::clearColor()));
                panel.setHasShadow(false);
                panel.setHidesOnDeactivate(false);
                panel.setCollectionBehavior(
                    NSWindowCollectionBehavior::CanJoinAllSpaces
                        | NSWindowCollectionBehavior::FullScreenAuxiliary
                        | NSWindowCollectionBehavior::Stationary,
                );
                panel.setLevel(OVERLAY_LEVEL);
                let view = SelectionView::new(mtm, NSRect::new(NSPoint::ZERO, frame.size));
                panel.setContentView(Some(&view));
                panel.setFrame_display(frame, true);
                panel.orderFrontRegardless();
                panel
            })
            .collect();
        NSCursor::crosshairCursor().push();
        Self { panels }
    }
}

impl Drop for RegionPicker {
    fn drop(&mut self) {
        // pop 是出栈当前光标（不管调谁），与 begin 里的 push 配对
        NSCursor::crosshairCursor().pop();
        for panel in &self.panels {
            panel.orderOut(None);
        }
    }
}

fn primary_height(mtm: MainThreadMarker) -> Option<f64> {
    NSScreen::screens(mtm)
        .firstObject()
        .map(|screen| screen.frame().size.height)
}
