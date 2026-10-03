//! 上屏缓冲：把刚上屏的中文攒成一句，停顿够久了才交给教练。纯逻辑，不碰 AppKit。

use std::time::{Duration, Instant};

/// 缓冲最多攒这么多字符，再长就不是一句话了，丢掉重来。
const MAX_CHARS: usize = 400;

#[derive(Debug, Default)]
pub struct TypedBuffer {
    text: String,

    last_commit: Option<Instant>,

    /// 这一句已经交给教练了；下一次上屏算新的一句。
    submitted: bool,
}

/// 一次上屏对缓冲的影响。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Note {
    /// 接在当前这句后面。
    Appended,

    /// 上一句已交出去，这是新的一句。
    StartedNew,

    /// 不是中文（英文字母、数字…），当前这句作废。
    Reset,
}

/// 中文输入里随汉字一起上屏的标点。
const CJK_PUNCTUATION: &str = "，。？！、；：“”‘’（）《》「」『』…—～·";

impl TypedBuffer {
    /// 记一次上屏。汉字与中文标点接在后面；别的内容说明用户去写别的东西了，缓冲清空。
    pub fn note(&mut self, text: &str) -> Note {
        self.note_at(text, Instant::now())
    }

    fn note_at(&mut self, text: &str, now: Instant) -> Note {
        let chinese = text.chars().any(is_cjk)
            && text.chars().all(|c| is_cjk(c) || CJK_PUNCTUATION.contains(c) || c.is_whitespace());
        let punctuation = !text.is_empty() && text.chars().all(|c| CJK_PUNCTUATION.contains(c));
        if !chinese && !punctuation {
            self.reset();
            return Note::Reset;
        }
        let started_new = std::mem::take(&mut self.submitted);
        if started_new || self.text.chars().count() + text.chars().count() > MAX_CHARS {
            self.text.clear();
        }
        self.text.push_str(text);
        self.last_commit = Some(now);
        if started_new { Note::StartedNew } else { Note::Appended }
    }

    /// 停顿满 `idle` 且还没交出去的这一句。
    pub fn ready(&self, idle: Duration) -> Option<&str> {
        self.ready_at(idle, Instant::now())
    }

    fn ready_at(&self, idle: Duration, now: Instant) -> Option<&str> {
        let last = self.last_commit?;
        (!self.submitted && !self.text.is_empty() && now.duration_since(last) >= idle)
            .then_some(self.text.as_str())
    }

    pub fn mark_submitted(&mut self) {
        self.submitted = true;
    }

    pub fn reset(&mut self) {
        self.text.clear();
        self.last_commit = None;
        self.submitted = false;
    }
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xF900..=0xFAFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDLE: Duration = Duration::from_millis(1300);

    #[test]
    fn collects_a_sentence_and_waits_for_the_pause() {
        let t0 = Instant::now();
        let mut buffer = TypedBuffer::default();
        assert_eq!(buffer.note_at("我这周", t0), Note::Appended);
        assert_eq!(buffer.note_at("实验做不完", t0 + Duration::from_millis(400)), Note::Appended);
        assert_eq!(buffer.ready_at(IDLE, t0 + Duration::from_millis(900)), None);
        assert_eq!(buffer.ready_at(IDLE, t0 + Duration::from_millis(1800)), Some("我这周实验做不完"));
    }

    #[test]
    fn chinese_punctuation_stays_in_the_sentence_but_latin_text_resets() {
        let t0 = Instant::now();
        let mut buffer = TypedBuffer::default();
        buffer.note_at("好的", t0);
        assert_eq!(buffer.note_at("，", t0), Note::Appended);
        assert_eq!(buffer.note_at("hello", t0), Note::Reset);
        assert_eq!(buffer.ready_at(Duration::ZERO, t0), None);
    }

    #[test]
    fn submitted_sentence_is_not_offered_twice_and_next_commit_starts_fresh() {
        let t0 = Instant::now();
        let mut buffer = TypedBuffer::default();
        buffer.note_at("你好吗", t0);
        buffer.mark_submitted();
        assert_eq!(buffer.ready_at(Duration::ZERO, t0), None);
        assert_eq!(buffer.note_at("明天见", t0), Note::StartedNew);
        assert_eq!(buffer.ready_at(Duration::ZERO, t0), Some("明天见"));
    }

    #[test]
    fn runaway_buffer_is_dropped() {
        let t0 = Instant::now();
        let mut buffer = TypedBuffer::default();
        for _ in 0..50 {
            buffer.note_at("这是一段很长很长的话", t0);
        }
        assert!(buffer.ready_at(Duration::ZERO, t0).unwrap().chars().count() <= MAX_CHARS);
    }
}
