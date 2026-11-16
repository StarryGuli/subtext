//! 言外窗口：模式（读 / 写 / 改）手动选，一个输入框，一个结果区。菜单栏图标与全局热键都只是把它叫出来。

use std::cell::RefCell;
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSAutoresizingMaskOptions, NSBackingStoreType, NSBezelStyle, NSButton,
    NSEventModifierFlags, NSMenu, NSMenuItem, NSPasteboard, NSPasteboardTypeString, NSScrollView,
    NSSegmentSwitchTracking, NSSegmentedControl, NSStatusBar, NSStatusItem, NSTextField,
    NSTextView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{
    NSArray, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSTimer,
};
use subtext_coach::gate::{Skip, language};
use subtext_coach::{
    CoachConfig, CoachContext, CoachEvent, CoachOutput, CoachRequest, CoachService, Mode,
    SharedCache,
};

use crate::attributed;
use crate::doc::Doc;

const WIDTH: f64 = 560.0;
const HEIGHT: f64 = 700.0;
const MARGIN: f64 = 16.0;

/// 写模式接对方语气用的「刚读过的那条」多久内有效。
const PEER_TTL: Duration = Duration::from_secs(30 * 60);

/// 读完一条之后，轮询结果的间隔（秒）。
const POLL_INTERVAL: f64 = 0.1;

thread_local! {
    static UI: RefCell<Option<Ui>> = const { RefCell::new(None) };
}

/// 在界面状态上做点事；重入（动作里又触发动作）时放弃而不是 panic。
fn with<R>(job: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    UI.with(|cell| cell.try_borrow_mut().ok()?.as_mut().map(job))
}

struct Ui {
    window: Retained<NSWindow>,
    segments: Retained<NSSegmentedControl>,
    input: Retained<NSTextView>,
    output: Retained<NSTextView>,
    status: Retained<NSTextField>,
    /// 菜单栏图标；丢掉它图标就没了。
    _item: Retained<NSStatusItem>,
    _target: Retained<UiTarget>,
    _timer: Retained<NSTimer>,
    service: CoachService,
    backend: String,
    next_id: u64,
    pending: Option<u64>,
    /// 最近读过的一条对方消息，写回复时接它的语气。
    last_read: Option<(String, Instant)>,
    /// 当前结果里「复制」按钮该复制的那段文字。
    copy_text: String,
    /// 上次带进输入框的剪贴板内容，同一份不重复覆盖用户正在改的输入。
    seen_clipboard: String,
}

define_class!(
    // SAFETY: NSObject 没有子类化要求；没有实现 Drop。
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    struct UiTarget;

    impl UiTarget {
        #[unsafe(method(submit:))]
        fn submit(&self, _sender: Option<&AnyObject>) {
            with(Ui::submit);
        }

        #[unsafe(method(copyResult:))]
        fn copy_result(&self, _sender: Option<&AnyObject>) {
            with(|ui| ui.copy_result());
        }

        #[unsafe(method(pasteClipboard:))]
        fn paste_clipboard(&self, _sender: Option<&AnyObject>) {
            with(|ui| ui.load_clipboard(true));
        }

        #[unsafe(method(clearAll:))]
        fn clear_all(&self, _sender: Option<&AnyObject>) {
            with(Ui::clear);
        }

        #[unsafe(method(showWindow:))]
        fn show_window(&self, _sender: Option<&AnyObject>) {
            present();
        }

        #[unsafe(method(tick:))]
        fn tick(&self, _timer: Option<&AnyObject>) {
            with(Ui::poll);
        }
    }

    unsafe impl NSObjectProtocol for UiTarget {}
);

impl UiTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

/// 建窗口、菜单栏图标与轮询定时器。
pub fn init(mtm: MainThreadMarker, config: &CoachConfig) {
    let target = UiTarget::new(mtm);
    let window = build_window(mtm);
    let content = window.contentView().expect("window has a content view");

    let top = |height: f64, used: f64| HEIGHT - MARGIN - used - height;
    let labels = NSArray::from_retained_slice(&[
        NSString::from_str("读  Read"),
        NSString::from_str("写  Write"),
        NSString::from_str("改  Edit"),
    ]);
    let segments = unsafe {
        NSSegmentedControl::segmentedControlWithLabels_trackingMode_target_action(
            &labels,
            NSSegmentSwitchTracking::SelectOne,
            Some(&target),
            None,
            mtm,
        )
    };
    segments.setSelectedSegment(0);
    segments.setFrame(NSRect::new(
        NSPoint::new(MARGIN, top(26.0, 0.0)),
        NSSize::new(WIDTH - 2.0 * MARGIN, 26.0),
    ));
    segments.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMinYMargin,
    );
    content.addSubview(&segments);

    let (input_scroll, input) = scroll_text(
        mtm,
        NSRect::new(
            NSPoint::new(MARGIN, top(150.0, 36.0)),
            NSSize::new(WIDTH - 2.0 * MARGIN, 150.0),
        ),
        true,
    );
    input_scroll.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMinYMargin,
    );
    content.addSubview(&input_scroll);

    let button_y = top(28.0, 196.0);
    let mut x = MARGIN;
    let mut add_button = |title: &str, action: objc2::runtime::Sel, width: f64, primary: bool| {
        let button = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str(title),
                Some(&target),
                Some(action),
                mtm,
            )
        };
        button.setBezelStyle(NSBezelStyle::Push);
        button.setFrame(NSRect::new(
            NSPoint::new(x, button_y),
            NSSize::new(width, 28.0),
        ));
        button.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinYMargin);
        if primary {
            button.setKeyEquivalent(&NSString::from_str("\r"));
            button.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        }
        content.addSubview(&button);
        x += width + 8.0;
    };
    add_button("解析  ⌘↵", sel!(submit:), 110.0, true);
    add_button("粘贴剪贴板", sel!(pasteClipboard:), 100.0, false);
    add_button("复制结果", sel!(copyResult:), 90.0, false);
    add_button("清空", sel!(clearAll:), 64.0, false);

    let (output_scroll, output) = scroll_text(
        mtm,
        NSRect::new(
            NSPoint::new(MARGIN, MARGIN + 24.0),
            NSSize::new(WIDTH - 2.0 * MARGIN, button_y - 12.0 - (MARGIN + 24.0)),
        ),
        false,
    );
    output_scroll.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    content.addSubview(&output_scroll);

    let status = NSTextField::labelWithString(&NSString::from_str(""), mtm);
    status.setFrame(NSRect::new(
        NSPoint::new(MARGIN, MARGIN - 4.0),
        NSSize::new(WIDTH - 2.0 * MARGIN, 18.0),
    ));
    status.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMaxYMargin,
    );
    status.setTextColor(Some(&objc2_app_kit::NSColor::secondaryLabelColor()));
    content.addSubview(&status);

    let item = build_status_item(mtm, &target);
    let timer = unsafe {
        NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
            POLL_INTERVAL,
            &target,
            sel!(tick:),
            None,
            true,
        )
    };
    let backend = config.backend.label().to_owned();
    let cache = SharedCache::in_memory();
    let ui = Ui {
        window,
        segments,
        input,
        output,
        status,
        _item: item,
        _target: target,
        _timer: timer,
        service: CoachService::start_with_cache(config, cache),
        backend,
        next_id: 0,
        pending: None,
        last_read: None,
        copy_text: String::new(),
        seen_clipboard: String::new(),
    };
    ui.show_doc(&hint_doc());
    ui.set_status("");
    UI.with(|cell| *cell.borrow_mut() = Some(ui));
}

/// 把窗口叫出来：剪贴板里有新内容就带进输入框，并按语言选个起始模式（之后随你改）。
pub fn present() {
    with(Ui::present);
}

fn build_window(mtm: MainThreadMarker) -> Retained<NSWindow> {
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc::<NSWindow>(),
            NSRect::new(NSPoint::ZERO, NSSize::new(WIDTH, HEIGHT)),
            NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Miniaturizable
                | NSWindowStyleMask::Resizable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(&NSString::from_str("言外 Subtext"));
    window.setMinSize(NSSize::new(420.0, 480.0));
    window.center();
    window
}

fn build_status_item(mtm: MainThreadMarker, target: &UiTarget) -> Retained<NSStatusItem> {
    let item = NSStatusBar::systemStatusBar().statusItemWithLength(-1.0);
    if let Some(button) = item.button(mtm) {
        button.setTitle(&NSString::from_str("言"));
    }
    let menu = NSMenu::new(mtm);
    let open = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str(&format!("打开言外（{}）", crate::hotkey::LABEL)),
            Some(sel!(showWindow:)),
            &NSString::from_str(""),
        )
    };
    unsafe { open.setTarget(Some(target)) };
    menu.addItem(&open);
    let quit = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("退出"),
            Some(sel!(terminate:)),
            &NSString::from_str("q"),
        )
    };
    menu.addItem(&quit);
    item.setMenu(Some(&menu));
    item
}

/// 一个带滚动条的文字区；`editable` 为假时只读但可选中复制。
fn scroll_text(
    mtm: MainThreadMarker,
    frame: NSRect,
    editable: bool,
) -> (Retained<NSScrollView>, Retained<NSTextView>) {
    let scroll = NSScrollView::initWithFrame(mtm.alloc(), frame);
    scroll.setHasVerticalScroller(true);
    scroll.setBorderType(objc2_app_kit::NSBorderType::BezelBorder);
    let text = NSTextView::initWithFrame(mtm.alloc(), NSRect::new(NSPoint::ZERO, frame.size));
    text.setEditable(editable);
    text.setSelectable(true);
    text.setRichText(!editable);
    text.setVerticallyResizable(true);
    text.setHorizontallyResizable(false);
    text.setMaxSize(NSSize::new(f64::MAX, f64::MAX));
    text.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    text.setTextContainerInset(NSSize::new(8.0, 8.0));
    // SAFETY: 取文字容器，只读属性
    if let Some(container) = unsafe { text.textContainer() } {
        container.setWidthTracksTextView(true);
    }
    if editable {
        text.setFont(Some(&objc2_app_kit::NSFont::systemFontOfSize(14.0)));
    }
    scroll.setDocumentView(Some(&text));
    (scroll, text)
}

fn hint_doc() -> Doc {
    Doc::plain(
        "言外",
        "读：贴一段收到的英文，逐句看懂。\n写：打中文（或中英混着），出能直接发的英文。\n改：贴你自己写的英文，帮你检查。\n\n选好上面的模式，⌘↵ 解析。",
    )
}

impl Ui {
    fn mode(&self) -> Mode {
        match self.segments.selectedSegment() {
            1 => Mode::Compose,
            2 => Mode::Edit,
            _ => Mode::Decode,
        }
    }

    fn set_status(&self, extra: &str) {
        let text = if extra.is_empty() {
            self.backend.clone()
        } else {
            format!("{} · {extra}", self.backend)
        };
        self.status.setStringValue(&NSString::from_str(&text));
    }

    fn show_doc(&self, doc: &Doc) {
        // SAFETY: 取文字存储，随即整体替换内容
        if let Some(storage) = unsafe { self.output.textStorage() } {
            storage.setAttributedString(&attributed::build(doc));
        }
    }

    fn present(&mut self) {
        self.load_clipboard(false);
        let mtm = MainThreadMarker::new().expect("main thread");
        let app = NSApplication::sharedApplication(mtm);
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
        self.window.makeKeyAndOrderFront(None);
        self.window.makeFirstResponder(Some(&self.input));
    }

    /// 读剪贴板。`force` 为假时只在内容比上次新时才覆盖输入框。
    fn load_clipboard(&mut self, force: bool) {
        let pasteboard = NSPasteboard::generalPasteboard();
        // SAFETY: 只读 AppKit 导出的类型常量
        let Some(text) = (unsafe { pasteboard.stringForType(NSPasteboardTypeString) }) else {
            return;
        };
        let text = text.to_string();
        if text.trim().is_empty() || (!force && text == self.seen_clipboard) {
            return;
        }
        self.seen_clipboard = text.clone();
        self.input.setString(&NSString::from_str(text.trim()));
        let mode = if language::cjk_chars(&text) > 0 { 1 } else { 0 };
        self.segments.setSelectedSegment(mode);
    }

    fn clear(&mut self) {
        self.input.setString(&NSString::from_str(""));
        self.pending = None;
        self.copy_text.clear();
        self.show_doc(&hint_doc());
        self.set_status("");
        self.window.makeFirstResponder(Some(&self.input));
    }

    fn submit(&mut self) {
        let text = self.input.string().to_string().trim().to_owned();
        if let Err(skip) = self.service.gate().check_manual(&text) {
            self.show_doc(&Doc::failure("没有发出去", skip_reason(skip)));
            return;
        }
        let mode = self.mode();
        let mut context = CoachContext::default();
        match mode {
            Mode::Decode => self.last_read = Some((text.clone(), Instant::now())),
            Mode::Compose => {
                context.peer_message = self
                    .last_read
                    .as_ref()
                    .filter(|(_, at)| at.elapsed() < PEER_TTL)
                    .map(|(peer, _)| peer.clone());
            }
            _ => {}
        }
        self.next_id += 1;
        let id = self.next_id;
        let request = CoachRequest {
            id,
            mode,
            text,
            context,
        };
        match self.service.submit(request) {
            Ok(()) => {
                self.pending = Some(id);
                self.copy_text.clear();
                self.show_doc(&Doc::thinking(mode_title(mode)));
                self.set_status("思考中…");
            }
            Err(error) => self.show_doc(&Doc::failure("没有发出去", &error.to_string())),
        }
    }

    fn poll(&mut self) {
        for event in self.service.poll() {
            if Some(event.id()) != self.pending {
                continue;
            }
            match event {
                CoachEvent::Started { .. } => {}
                CoachEvent::Partial { output, .. } => self.show_doc(&doc_for(&output)),
                CoachEvent::Finished { output, cached, .. } => {
                    self.show_doc(&doc_for(&output));
                    self.copy_text = primary_text(&output);
                    self.pending = None;
                    self.set_status(if cached { "来自历史" } else { "完成" });
                }
                CoachEvent::Failed { mode, message, .. } => {
                    self.show_doc(&Doc::failure(mode_title(mode), &message));
                    self.pending = None;
                    self.set_status("失败");
                }
            }
        }
    }

    fn copy_result(&mut self) {
        if self.copy_text.is_empty() {
            self.set_status("还没有可复制的结果");
            return;
        }
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        // SAFETY: 只读 AppKit 导出的类型常量
        unsafe {
            pasteboard
                .setString_forType(&NSString::from_str(&self.copy_text), NSPasteboardTypeString);
        }
        // 自己写进去的不再当作「新复制」带回输入框
        self.seen_clipboard = self.copy_text.clone();
        self.set_status("已复制");
    }
}

fn mode_title(mode: Mode) -> &'static str {
    match mode {
        Mode::Decode => "读",
        Mode::Compose => "写",
        Mode::Edit => "改",
        Mode::Screen => "屏幕",
    }
}

fn skip_reason(skip: Skip) -> &'static str {
    match skip {
        Skip::Empty => "先输入或粘贴一点内容。",
        Skip::TooLong => "内容太长了，分段贴。",
        Skip::LooksLikeSecret => "里面像是有密钥或密码，为安全起见没有发出去。",
        Skip::MachineText => "这像是链接或代码，没有发出去。",
        _ => "这段内容没有发出去。",
    }
}

fn doc_for(output: &CoachOutput) -> Doc {
    match output {
        CoachOutput::Decode(decoded) => Doc::decode(decoded, true),
        CoachOutput::Compose(composed) => Doc::compose(composed),
        CoachOutput::Edit(edited) => Doc::edit(edited),
        CoachOutput::Screen(_) => Doc::default(),
        CoachOutput::Plain { mode, text } => Doc::plain(mode_title(*mode), text),
    }
}

/// 「复制结果」复制哪一段：写 / 改给可直接发送的英文，读给逐句译文。
fn primary_text(output: &CoachOutput) -> String {
    match output {
        CoachOutput::Compose(composed) => composed
            .options
            .iter()
            .find(|option| option.recommended)
            .or(composed.options.first())
            .map(|option| option.text.clone())
            .unwrap_or_default(),
        CoachOutput::Edit(edited) => edited.corrected.clone(),
        CoachOutput::Decode(decoded) => {
            let lines: Vec<&str> = decoded
                .lines
                .iter()
                .map(|line| line.zh.as_str())
                .filter(|zh| !zh.is_empty())
                .collect();
            if lines.is_empty() {
                decoded.translation.clone()
            } else {
                lines.join("\n")
            }
        }
        CoachOutput::Plain { text, .. } => text.clone(),
        CoachOutput::Screen(_) => String::new(),
    }
}

/// 开发预览：`subtext --snapshot <decode|compose|edit> <回复.json> <输入文字> <输出.png>`，
/// 把窗口渲染成 PNG 后退出。不要屏幕录制权限，也不发请求。
pub fn snapshot_if_requested() -> Option<i32> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let position = arguments.iter().position(|a| a == "--snapshot")?;
    let [mode, reply, input, output, ..] = &arguments[position + 1..] else {
        eprintln!("用法：--snapshot <decode|compose|edit> <回复.json> <输入文字> <输出.png>");
        return Some(2);
    };
    let (mode, segment) = match mode.as_str() {
        "decode" => (Mode::Decode, 0),
        "compose" => (Mode::Compose, 1),
        "edit" => (Mode::Edit, 2),
        other => {
            eprintln!("不认识的模式：{other}");
            return Some(2);
        }
    };
    let parsed = std::fs::read_to_string(reply)
        .map_err(|error| error.to_string())
        .and_then(|text| CoachOutput::parse(mode, &text).map_err(|error| error.to_string()));
    let parsed = match parsed {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("读不了回复：{error}");
            return Some(1);
        }
    };
    let ok = with(|ui| {
        ui.segments.setSelectedSegment(segment);
        ui.input.setString(&NSString::from_str(input));
        ui.show_doc(&doc_for(&parsed));
        ui.set_status("完成");
        let content = ui.window.contentView()?;
        content.layoutSubtreeIfNeeded();
        let bounds = content.bounds();
        let rep = content.bitmapImageRepForCachingDisplayInRect(bounds)?;
        content.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
        let data = unsafe {
            rep.representationUsingType_properties(
                objc2_app_kit::NSBitmapImageFileType::PNG,
                &objc2_foundation::NSDictionary::new(),
            )
        }?;
        std::fs::write(output, data.to_vec()).ok()
    })
    .flatten()
    .is_some();
    Some(if ok { 0 } else { 1 })
}
