//! 屏幕阅读的记忆：哪些块已经见过 / 分析过、哪些正在分析。OCR 每次的结果有细微抖动（标点、个别字），
//! 所以「同一块」靠相似度认，认出来就复用它的分析结果，不重复花额度。

use std::collections::{HashMap, HashSet};

use super::{Block, Rect};

/// 记住多少块；超过就忘最老的。
const CAPACITY: usize = 400;

/// 两块文字的相似度（字符二元组的 Dice 系数）达到这个值就当作同一块。
const SAME_BLOCK_SIMILARITY: f32 = 0.9;

/// 一块的分析结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Analysis {
    /// 译文：英文块译成中文。
    pub translation: String,

    /// 一句语气 / 俚语 / 潜台词提示，没有值得说的就空。
    pub note: String,
}

/// 屏幕上的一块，带着它的稳定标识与分析状态。
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenBlock {
    /// 稳定标识：跨多次 OCR 对同一块文字保持不变。
    pub key: String,

    pub text: String,

    pub rect: Rect,

    pub analysis: Option<Analysis>,

    /// 已经发出分析请求、还没回来。
    pub pending: bool,
}

#[derive(Debug, Default)]
pub struct ScreenMemory {
    /// 见过的块，按先后顺序；元素是 (规范化文字, 标识)。
    seen: Vec<(String, String)>,

    analyses: HashMap<String, Analysis>,

    pending: HashSet<String>,
}

impl ScreenMemory {
    /// 吃进一次 OCR 的块，返回带标识与分析状态的块（顺序不变）。
    pub fn ingest(&mut self, blocks: Vec<Block>) -> Vec<ScreenBlock> {
        blocks
            .into_iter()
            .map(|block| {
                let key = self.key_for(&block.text);
                ScreenBlock {
                    analysis: self.analyses.get(&key).cloned(),
                    pending: self.pending.contains(&key),
                    key,
                    text: block.text,
                    rect: block.rect,
                }
            })
            .collect()
    }

    /// 还没分析、也不在分析中、且 `eligible` 认可的块，最多 `limit` 个，按屏幕上从上到下（越新越靠下）。
    pub fn to_analyze(
        &self,
        blocks: &[ScreenBlock],
        limit: usize,
        eligible: impl Fn(&str) -> bool,
    ) -> Vec<(String, String)> {
        let mut wanted: Vec<(String, String)> = blocks
            .iter()
            .filter(|block| {
                !self.analyses.contains_key(&block.key)
                    && !self.pending.contains(&block.key)
                    && eligible(&block.text)
            })
            .map(|block| (block.key.clone(), block.text.clone()))
            .collect();
        // 多了先顾最新的（最靠下的）
        if wanted.len() > limit {
            wanted.drain(..wanted.len() - limit);
        }
        wanted
    }

    pub fn mark_pending(&mut self, keys: &[String]) {
        self.pending.extend(keys.iter().cloned());
    }

    pub fn clear_pending(&mut self, keys: &[String]) {
        for key in keys {
            self.pending.remove(key);
        }
    }

    pub fn store(&mut self, key: &str, analysis: Analysis) {
        self.analyses.insert(key.to_owned(), analysis);
    }

    pub fn analysis(&self, key: &str) -> Option<&Analysis> {
        self.analyses.get(key)
    }

    /// 全部清掉（换了阅读目标）。
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    fn key_for(&mut self, text: &str) -> String {
        let normalized = normalize(text);
        if let Some((_, key)) = self.seen.iter().find(|(known, _)| *known == normalized) {
            return key.clone();
        }
        if let Some((_, key)) = self
            .seen
            .iter()
            .find(|(known, _)| similar(known, &normalized))
        {
            return key.clone();
        }
        let key = normalized.clone();
        self.seen.push((normalized, key.clone()));
        if self.seen.len() > CAPACITY {
            let (_, dropped) = self.seen.remove(0);
            self.analyses.remove(&dropped);
            self.pending.remove(&dropped);
        }
        key
    }
}

/// 鼠标下面是哪一块：几块都包含这个点时取最小的（最具体的）。
pub fn block_at(blocks: &[ScreenBlock], x: f32, y: f32, slack: f32) -> Option<&ScreenBlock> {
    blocks
        .iter()
        .filter(|block| block.rect.contains(x, y, slack))
        .min_by(|a, b| (a.rect.width * a.rect.height).total_cmp(&(b.rect.width * b.rect.height)))
}

/// 去掉空白与标点、转小写，只留字母数字和汉字：对比用。
fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// 两段规范化文字够不够像：长度接近、字符二元组重合度高，且数字不矛盾。
/// 数字要一致：`see you at 5pm` 与 `see you at 6pm` 只差一个字符，却是两条不同的消息，不能共用译文。
fn similar(a: &str, b: &str) -> bool {
    let (a_len, b_len) = (a.chars().count(), b.chars().count());
    if a_len < 8 || b_len < 8 {
        return false;
    }
    let digits = |text: &str| -> String { text.chars().filter(char::is_ascii_digit).collect() };
    let (a_digits, b_digits) = (digits(a), digits(b));
    // 两边都有数字却不一样：是两条消息。只一边多出一个孤立数字（`?` 被识成 `7`）：当作识别抖动
    let jitter =
        a_digits.is_empty() != b_digits.is_empty() && a_digits.len().max(b_digits.len()) <= 1;
    if a_digits != b_digits && !jitter {
        return false;
    }
    let ratio = a_len.min(b_len) as f32 / a_len.max(b_len) as f32;
    ratio >= 0.8 && dice(a, b) >= SAME_BLOCK_SIMILARITY
}

fn dice(a: &str, b: &str) -> f32 {
    let bigrams = |text: &str| -> HashMap<(char, char), usize> {
        let chars: Vec<char> = text.chars().collect();
        let mut counts = HashMap::new();
        for pair in chars.windows(2) {
            *counts.entry((pair[0], pair[1])).or_insert(0) += 1;
        }
        counts
    };
    let (left, right) = (bigrams(a), bigrams(b));
    let total: usize = left.values().sum::<usize>() + right.values().sum::<usize>();
    if total == 0 {
        return 0.0;
    }
    let shared: usize = left
        .iter()
        .map(|(pair, count)| (*count).min(right.get(pair).copied().unwrap_or(0)))
        .sum();
    2.0 * shared as f32 / total as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(text: &str, y: f32) -> Block {
        Block {
            text: text.to_owned(),
            rect: Rect::new(0.0, y, 100.0, 20.0),
            lines: 1,
        }
    }

    #[test]
    fn ocr_jitter_keeps_the_same_key_and_the_analysis() {
        let mut memory = ScreenMemory::default();
        let first = memory.ingest(vec![block(
            "Hey, no rush at all but did you look at that PR?",
            10.0,
        )]);
        memory.store(
            &first[0].key,
            Analysis {
                translation: "嘿，不着急……".to_owned(),
                note: String::new(),
            },
        );
        // 下一次 OCR 把 PR? 识成 PR7，并多了个空格
        let second = memory.ingest(vec![block(
            "Hey,  no rush at all but did you look at that PR7",
            14.0,
        )]);
        assert_eq!(second[0].key, first[0].key);
        assert_eq!(
            second[0].analysis.as_ref().unwrap().translation,
            "嘿，不着急……"
        );
    }

    #[test]
    fn messages_that_differ_only_in_a_number_are_different() {
        let mut memory = ScreenMemory::default();
        let blocks = memory.ingest(vec![
            block("ok see you at the lab at 5pm", 10.0),
            block("ok see you at the lab at 6pm", 40.0),
        ]);
        assert_ne!(blocks[0].key, blocks[1].key);
    }

    #[test]
    fn different_messages_get_different_keys() {
        let mut memory = ScreenMemory::default();
        let blocks = memory.ingest(vec![
            block("see you at the lab tomorrow morning", 10.0),
            block("see you at the lab tomorrow evening", 40.0),
            block("ok", 70.0),
            block("no", 100.0),
        ]);
        let keys: HashSet<&String> = blocks.iter().map(|b| &b.key).collect();
        // 前两条只差一个词，相似度没到阈值之上就必须分开；短消息永远靠精确匹配
        assert!(keys.len() >= 3, "{keys:?}");
        assert_ne!(blocks[2].key, blocks[3].key);
    }

    #[test]
    fn only_unanalyzed_unpending_eligible_blocks_are_queued_newest_last() {
        let mut memory = ScreenMemory::default();
        let blocks = memory.ingest(vec![
            block("first english message", 10.0),
            block("second english message", 40.0),
            block("third english message", 70.0),
            block("你好吗", 100.0),
        ]);
        memory.store(&blocks[0].key, Analysis::default());
        memory.mark_pending(&[blocks[1].key.clone()]);
        let wanted = memory.to_analyze(&blocks, 8, |text| text.is_ascii());
        assert_eq!(wanted.len(), 1);
        assert_eq!(wanted[0].1, "third english message");
        // limit 小于待办数时留最新的
        let fresh = ScreenMemory::default();
        let many = vec![
            ScreenBlock {
                key: "a".into(),
                text: "a".into(),
                rect: Rect::default(),
                analysis: None,
                pending: false,
            },
            ScreenBlock {
                key: "b".into(),
                text: "b".into(),
                rect: Rect::default(),
                analysis: None,
                pending: false,
            },
            ScreenBlock {
                key: "c".into(),
                text: "c".into(),
                rect: Rect::default(),
                analysis: None,
                pending: false,
            },
        ];
        let kept = fresh.to_analyze(&many, 2, |_| true);
        assert_eq!(
            kept.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
            ["b", "c"]
        );
    }

    #[test]
    fn pending_flag_clears_and_stores_show_up_on_next_ingest() {
        let mut memory = ScreenMemory::default();
        let blocks = memory.ingest(vec![block("a message to translate", 10.0)]);
        memory.mark_pending(&[blocks[0].key.clone()]);
        assert!(memory.ingest(vec![block("a message to translate", 10.0)])[0].pending);
        memory.clear_pending(&[blocks[0].key.clone()]);
        memory.store(
            &blocks[0].key,
            Analysis {
                translation: "x".into(),
                note: String::new(),
            },
        );
        let again = memory.ingest(vec![block("a message to translate", 10.0)]);
        assert!(!again[0].pending && again[0].analysis.is_some());
    }

    #[test]
    fn hover_picks_the_smallest_block_under_the_pointer() {
        let big = ScreenBlock {
            key: "big".into(),
            text: "big".into(),
            rect: Rect::new(0.0, 0.0, 400.0, 400.0),
            analysis: None,
            pending: false,
        };
        let small = ScreenBlock {
            key: "small".into(),
            text: "small".into(),
            rect: Rect::new(50.0, 50.0, 100.0, 20.0),
            analysis: None,
            pending: false,
        };
        let blocks = [big, small];
        assert_eq!(block_at(&blocks, 60.0, 55.0, 2.0).unwrap().key, "small");
        assert_eq!(block_at(&blocks, 300.0, 300.0, 2.0).unwrap().key, "big");
        assert!(block_at(&blocks, 900.0, 900.0, 2.0).is_none());
    }

    #[test]
    fn forgets_the_oldest_beyond_capacity() {
        let mut memory = ScreenMemory::default();
        let first = memory.ingest(vec![block("the very first message", 0.0)]);
        memory.store(&first[0].key, Analysis::default());
        for index in 0..CAPACITY {
            memory.ingest(vec![block(
                &format!("filler message number {index:04} zzz{index}"),
                0.0,
            )]);
        }
        assert!(memory.analysis(&first[0].key).is_none());
    }
}
