//! 触发：复制了英文 → 解码；上屏了一句中文并停顿 → 组句。每次轮询（0.3 秒）看一遍。

use std::time::Duration;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::NSWorkspace;
use objc2_foundation::NSRect;
use subtext_coach::{CoachContext, CoachRequest, Mode, SharedCache, Trigger};

use super::{Coach, ReplaceTarget, Shown, clipboard};
use crate::imk::secure_input;

/// 上屏后停顿多久，才把攒下的中文当作写完的一句。
const COMPOSE_IDLE: Duration = Duration::from_millis(1400);

/// 轮询交给调用方的活：要去应用里确认这句中文的位置（碰客户端，必须在借用 Host 之外做）。
pub struct ComposeProbe {
    pub client: Retained<AnyObject>,

    pub text: String,

    pub app: Option<String>,

    pub anchor: NSRect,
}

/// 打完一句英文停顿够久了：要去应用里读出光标前那一句（碰客户端，必须在借用 Host 之外做）。
pub struct EnglishProbe {
    pub client: Retained<AnyObject>,

    pub app: Option<String>,
}

/// 轮询交给调用方的活。
pub enum Probe {
    Compose(ComposeProbe),

    English(EnglishProbe),
}

/// 打完英文后停顿多久，才把光标前那一句当作写完了。
const ENGLISH_IDLE: Duration = Duration::from_millis(1800);

impl Coach {
    /// 每 0.3 秒一次。返回的探测任务由调用方在 Host 借用之外完成，再回调
    /// [`Self::submit_compose`] / [`Self::submit_english`]。
    pub fn tick(&mut self) -> Option<Probe> {
        self.process_events();
        self.expire();
        self.service.as_ref()?;
        self.refresh_source();
        if !self.source_ours {
            return None;
        }
        // 密码框：连缓冲里的中文也丢掉，更不读剪贴板
        if secure_input::enabled() {
            self.typed.reset();
            self.clipboard.reset();
            return None;
        }
        self.poll_clipboard();
        self.compose_candidate()
            .map(Probe::Compose)
            .or_else(|| self.english_candidate().map(Probe::English))
    }

    /// 打完英文停顿够久了：交给调用方去读光标前那一句。
    fn english_candidate(&mut self) -> Option<EnglishProbe> {
        if !self.config.auto_edit {
            return None;
        }
        let since = self.english_dirty?;
        if since.elapsed() < ENGLISH_IDLE {
            return None;
        }
        self.english_dirty = None;
        Some(EnglishProbe {
            client: self.last_client.clone()?,
            app: self.last_app.clone(),
        })
    }

    /// 调用方读到了光标前那一句（`None` 表示读不到或不是一句话）：过闸门，通过就发出改稿请求，面板跟着光标出现。
    pub fn submit_english(
        &mut self,
        probe: EnglishProbe,
        located: Option<(String, objc2_foundation::NSRange, NSRect)>,
    ) {
        let Some((text, range, anchor)) = located else {
            return;
        };
        let Some(service) = &self.service else {
            return;
        };
        // 同一句话已经校对过就别再弹
        if self.last_edit_text.as_deref() == Some(text.as_str()) {
            return;
        }
        if service
            .gate()
            .check(Trigger::EnglishDraft, &text, probe.app.as_deref(), false)
            .is_err()
        {
            return;
        }
        let id = self.next_id + 1;
        let request = CoachRequest {
            id,
            mode: Mode::Edit,
            text: text.clone(),
            context: CoachContext {
                app: probe.app,
                before: String::new(),
                peer_message: self.memory.peer_message().map(str::to_owned),
            },
        };
        if service.submit(request).is_err() {
            return;
        }
        self.next_id = id;
        self.last_edit_text = Some(text.clone());
        self.begin(Shown {
            id,
            mode: Mode::Edit,
            source: text.clone(),
            output: None,
            failure: None,
            revealed: false,
            streaming: false,
            anchor,
            target: Some(ReplaceTarget {
                client: probe.client,
                range,
                expected: text,
            }),
            shown_at: std::time::Instant::now(),
            note: None,
        });
    }

    /// 剪贴板里有新复制：过闸门，通过就解码。
    fn poll_clipboard(&mut self) {
        let Some(copied) = self.clipboard.poll() else {
            return;
        };
        if !self.config.auto_decode {
            return;
        }
        let app = frontmost_application();
        let Some(service) = &self.service else {
            return;
        };
        if let Err(skip) = service.gate().check(
            Trigger::ClipboardCopy,
            &copied.text,
            app.as_deref(),
            copied.concealed,
        ) {
            tracing::debug!(?skip, "复制的内容不交给教练");
            return;
        }
        let text = copied.text.trim().to_owned();
        self.next_id += 1;
        let id = self.next_id;
        let request = CoachRequest {
            id,
            mode: Mode::Decode,
            text: text.clone(),
            context: CoachContext {
                app,
                before: String::new(),
                peer_message: None,
            },
        };
        if service.submit(request).is_err() {
            tracing::warn!("教练线程已停止，解码请求没有发出");
            return;
        }
        self.begin(Shown {
            id,
            mode: Mode::Decode,
            source: text,
            output: None,
            failure: None,
            revealed: false,
            streaming: false,
            anchor: NSRect::ZERO,
            target: None,
            shown_at: std::time::Instant::now(),
            note: None,
        });
    }

    /// 攒的中文停顿够久了：过闸门，通过就交给调用方去确认位置。
    fn compose_candidate(&mut self) -> Option<ComposeProbe> {
        if !self.config.auto_compose {
            return None;
        }
        let text = self.typed.ready(COMPOSE_IDLE)?.to_owned();
        let service = self.service.as_ref()?;
        let app = self.last_app.clone();
        match service
            .gate()
            .check(Trigger::ChineseCommitted, &text, app.as_deref(), false)
        {
            Ok(()) => {}
            // 太短就继续等：用户可能还在写这一句
            Err(subtext_coach::gate::Skip::TooShort) => return None,
            Err(skip) => {
                tracing::debug!(?skip, "上屏的中文不交给教练");
                self.typed.mark_submitted();
                return None;
            }
        }
        self.typed.mark_submitted();
        Some(ComposeProbe {
            client: self.last_client.clone()?,
            text,
            app,
            anchor: self.last_anchor,
        })
    }

    /// 调用方确认了位置（`range` 为 `None` 表示找不到，只能复制）：发出组句请求，面板跟着光标出现。
    pub fn submit_compose(
        &mut self,
        probe: ComposeProbe,
        range: Option<objc2_foundation::NSRange>,
    ) {
        let Some(service) = &self.service else {
            return;
        };
        let id = self.next_id + 1;
        let request = CoachRequest {
            id,
            mode: Mode::Compose,
            text: probe.text.clone(),
            context: CoachContext {
                app: probe.app,
                before: String::new(),
                peer_message: self.memory.peer_message().map(str::to_owned),
            },
        };
        if service.submit(request).is_err() {
            return;
        }
        self.next_id = id;
        let target = range.map(|range| ReplaceTarget {
            client: probe.client,
            range,
            expected: probe.text.clone(),
        });
        self.begin(Shown {
            id,
            mode: Mode::Compose,
            source: probe.text,
            output: None,
            failure: None,
            revealed: false,
            streaming: false,
            anchor: probe.anchor,
            target,
            shown_at: std::time::Instant::now(),
            note: None,
        });
    }

    /// 刚上屏了一段文字：中文接进缓冲，别的内容让缓冲作废；上屏说明用户在继续写，正显示的组句面板收起。
    pub fn note_commit(
        &mut self,
        text: &str,
        client: &AnyObject,
        app: Option<String>,
        anchor: NSRect,
    ) {
        if self.service.is_none() {
            return;
        }
        self.typed.note(text);
        if text.chars().any(|c| c.is_ascii_alphabetic()) {
            self.english_dirty = Some(std::time::Instant::now());
        }
        // SAFETY: client 是 IMK 传进来的有效对象，retain 之后自己持有一份引用
        self.last_client = unsafe { Retained::retain(std::ptr::from_ref(client).cast_mut()) };
        self.last_app = app;
        self.last_anchor = anchor;
    }
}

/// 最前面那个应用的 bundle id，给闸门的「不处理这些应用」与语域推断用。
fn frontmost_application() -> Option<String> {
    NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .and_then(|app| app.bundleIdentifier())
        .map(|id| id.to_string())
}

impl Coach {
    /// 把应用里选中的文字交给教练：英文先当自己写的草稿改稿，英文太像别人发来的也按改稿处理；
    /// 中文给出英文表达。返回 `false` 表示闸门没放行或教练线程不在。
    pub fn submit_selection(
        &mut self,
        text: &str,
        range: objc2_foundation::NSRange,
        client: &AnyObject,
        app: Option<String>,
        anchor: NSRect,
    ) -> bool {
        let Some(service) = &self.service else {
            return false;
        };
        let (trigger, mode) = [Trigger::EnglishDraft, Trigger::ChineseCommitted]
            .into_iter()
            .find_map(|trigger| trigger.mode_for(text).map(|mode| (trigger, mode)))
            .unzip();
        let (Some(trigger), Some(mode)) = (trigger, mode) else {
            return false;
        };
        if service
            .gate()
            .check(trigger, text, app.as_deref(), false)
            .is_err()
        {
            return false;
        }
        self.next_id += 1;
        let id = self.next_id;
        let request = CoachRequest {
            id,
            mode,
            text: text.trim().to_owned(),
            context: CoachContext {
                app,
                before: String::new(),
                peer_message: self.memory.peer_message().map(str::to_owned),
            },
        };
        if service.submit(request).is_err() {
            return false;
        }
        // SAFETY: client 是 IMK 传进来的有效对象，retain 之后自己持有一份引用
        let retained = unsafe { Retained::retain(std::ptr::from_ref(client).cast_mut()) };
        self.begin(Shown {
            id,
            mode,
            source: text.to_owned(),
            output: None,
            failure: None,
            revealed: false,
            streaming: false,
            anchor,
            target: retained.map(|client| ReplaceTarget {
                client,
                range,
                expected: text.to_owned(),
            }),
            shown_at: std::time::Instant::now(),
            note: None,
        });
        true
    }
}

impl Coach {
    /// 用户要求「现在就解析剪贴板里的内容」：不管开没开自动、有没有解析过，英文解码、中文给出英文，
    /// 并忽略缓存重新来一遍。返回要告诉用户的话（成功是 `None`）。
    pub fn submit_clipboard_now(&mut self) -> Option<String> {
        let Some(service) = &self.service else {
            return Some("双语教练没开".to_owned());
        };
        let Some(text) = clipboard::read_now() else {
            return Some("剪贴板里没有可解析的文字".to_owned());
        };
        let text = text.trim().to_owned();
        let (trigger, mode) = [Trigger::ClipboardCopy, Trigger::ChineseCommitted]
            .into_iter()
            .find_map(|trigger| trigger.mode_for(&text).map(|mode| (trigger, mode)))
            .unzip();
        let (Some(trigger), Some(mode)) = (trigger, mode) else {
            return Some("剪贴板里的内容不是英文也不是中文句子".to_owned());
        };
        let app = frontmost_application();
        if service
            .gate()
            .check(trigger, &text, app.as_deref(), false)
            .is_err()
        {
            return Some("这段文字不适合交给教练（太长、疑似密钥或链接代码）".to_owned());
        }
        self.next_id += 1;
        let id = self.next_id;
        let request = CoachRequest {
            id,
            mode,
            text: text.clone(),
            context: CoachContext {
                app,
                before: String::new(),
                peer_message: (mode == Mode::Compose)
                    .then(|| self.memory.peer_message().map(str::to_owned))
                    .flatten(),
            },
        };
        self.cache.forget(SharedCache::key(&request));
        if service.submit(request).is_err() {
            return Some("教练线程已停止".to_owned());
        }
        self.begin(Shown {
            id,
            mode,
            source: text,
            output: None,
            failure: None,
            revealed: false,
            streaming: false,
            anchor: NSRect::ZERO,
            target: None,
            shown_at: std::time::Instant::now(),
            note: None,
        });
        None
    }
}
