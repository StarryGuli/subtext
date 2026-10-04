//! Active 往后端发的请求：批量翻译、鼠标指着的块插队、空闲时的完整解码预热。

use std::time::Instant;

use subtext_coach::screen::ScreenBlock;
use subtext_coach::{CoachContext, CoachRequest, Mode};

use super::{Active, BATCH, CONTEXT_BLOCKS, MAX_IN_FLIGHT};

impl Active {
    /// 屏幕上有新的英文块就批量交给后端；鼠标正指着的那块没翻译就插队。
    pub(super) fn submit_translations(&mut self) {
        if self.in_flight.len() >= MAX_IN_FLIGHT || Instant::now() < self.retry_after {
            return;
        }
        let app = self.app();
        let gate = &self.gate;
        let mut wanted = self.memory.to_analyze(&self.blocks, BATCH, |text| {
            gate.allow_screen_block(text, app.as_deref())
        });
        // 鼠标指着的块没翻译：放最前面，哪怕已经有一批在飞
        if let Some(hovered) = self.hovered_block()
            && hovered.analysis.is_none()
            && !hovered.pending
            && self.gate.allow_screen_block(&hovered.text, app.as_deref())
            && !wanted.iter().any(|(key, _)| *key == hovered.key)
        {
            wanted.insert(0, (hovered.key.clone(), hovered.text.clone()));
            wanted.truncate(BATCH);
        }
        if wanted.is_empty() || (self.in_flight.len() == 1 && !self.hover_needs_priority()) {
            return;
        }
        let keys: Vec<String> = wanted.iter().map(|(key, _)| key.clone()).collect();
        let numbered: String = wanted
            .iter()
            .enumerate()
            .map(|(index, (_, text))| format!("{}. {text}", index + 1))
            .collect::<Vec<_>>()
            .join("\n");
        let first = self
            .blocks
            .iter()
            .position(|block| block.key == keys[0])
            .unwrap_or(0);
        let before = self.context_before(first);
        let id = self.next_request_id();
        let request = CoachRequest {
            id,
            mode: Mode::Screen,
            text: numbered,
            context: CoachContext {
                app,
                before,
                peer_message: None,
            },
        };
        if self.service.submit(request).is_ok() {
            self.memory.mark_pending(&keys);
            self.in_flight.insert(id, keys);
        }
    }

    /// 翻译都排完了、后端空着的时候，替最新的几块提前做完整解码：
    /// 结果进共享缓存，之后复制同一段文字或悬停久了都是瞬间出现。一次只做一块。
    pub(super) fn submit_prewarm(&mut self) {
        if self.prewarm == 0
            || !self.in_flight.is_empty()
            || !self.warming.is_empty()
            || Instant::now() < self.retry_after
        {
            return;
        }
        let app = self.app();
        let hovered = self.hover_key.clone();
        let candidates: Vec<(usize, &ScreenBlock)> = self
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, block)| {
                block.analysis.is_some()
                    && self.gate.allow_screen_block(&block.text, app.as_deref())
            })
            .collect();
        let newest = candidates.len().saturating_sub(self.prewarm);
        let pick = candidates
            .iter()
            .skip(newest)
            .rev()
            .chain(
                candidates
                    .iter()
                    .take(newest)
                    .filter(|(_, block)| hovered.as_deref() == Some(block.key.as_str())),
            )
            .find(|(_, block)| {
                !self.decoded.contains_key(&block.key) && !self.warm_failed.contains(&block.key)
            })
            .map(|(index, block)| (*index, block.key.clone(), block.text.clone()));
        let Some((index, key, text)) = pick else {
            return;
        };
        let before = self.context_before(index);
        let id = self.next_request_id();
        let request = CoachRequest {
            id,
            mode: Mode::Decode,
            text,
            context: CoachContext {
                app,
                before,
                peer_message: None,
            },
        };
        if self.service.submit(request).is_ok() {
            self.warming.insert(id, key);
        } else {
            self.warm_failed.insert(key);
        }
    }

    fn next_request_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// 第 `first` 块之前最多几块的文字，给模型当上文。
    fn context_before(&self, first: usize) -> String {
        self.blocks[first.saturating_sub(CONTEXT_BLOCKS)..first]
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 已经有一个批量请求在飞时，只有鼠标指着的块没翻译才值得再发一个插队的。
    fn hover_needs_priority(&self) -> bool {
        self.hovered_block()
            .is_some_and(|block| block.analysis.is_none() && !block.pending)
    }

    pub(super) fn hovered_block(&self) -> Option<&ScreenBlock> {
        let key = self.hover_key.as_ref()?;
        self.blocks.iter().find(|block| block.key == *key)
    }
}
