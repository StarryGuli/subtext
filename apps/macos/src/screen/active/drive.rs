//! Active 的推进：取扫描结果、按时发起扫描、处理后端事件。

use std::time::Instant;

use subtext_coach::screen::{Analysis, group_blocks};
use subtext_coach::{CoachEvent, CoachOutput};

use super::{Active, DECODED_LIMIT, GONE_LIMIT, MAX_FAILURES, RETRY_DELAY};
use crate::screen::scan::{ScanJob, ScanOutcome};

impl Active {
    /// 推进一步；返回 `true` 表示目标已经长时间读不到，该停了。
    pub(in crate::screen) fn drive(&mut self) -> bool {
        while let Some(outcome) = self.scanner.poll() {
            match outcome {
                ScanOutcome::Lines { lines, bounds } => {
                    self.bounds = bounds;
                    self.last_error = None;
                    self.gone_since = None;
                    let blocks = group_blocks(&lines);
                    self.blocks = self.memory.ingest(blocks);
                }
                ScanOutcome::Unchanged { bounds } => {
                    self.bounds = bounds;
                    self.last_error = None;
                    self.gone_since = None;
                }
                ScanOutcome::Gone => {
                    self.gone_since.get_or_insert_with(Instant::now);
                }
                ScanOutcome::Failed(message) => self.last_error = Some(message),
            }
        }
        if self
            .gone_since
            .is_some_and(|since| since.elapsed() >= GONE_LIMIT)
        {
            return true;
        }
        if !self.scanner.is_busy() && self.last_scan.elapsed() >= self.interval {
            self.last_scan = Instant::now();
            self.scanner.submit(ScanJob {
                target: self.target.clone(),
                helper: self.helper.clone(),
                fake_image: self.fake_image.clone(),
            });
        }
        self.process_events();
        self.submit_translations();
        self.submit_prewarm();
        false
    }

    /// 后端的结果：流式中途的前面几条先存下，最后一条可能还没写完，等 Finished。
    fn process_events(&mut self) {
        for event in self.service.poll() {
            let id = event.id();
            if let Some(key) = self.warming.get(&id).cloned() {
                self.finish_warm(id, &key, event);
                continue;
            }
            let Some(keys) = self.in_flight.get(&id).cloned() else {
                continue;
            };
            match event {
                CoachEvent::Partial {
                    output: CoachOutput::Screen(screened),
                    ..
                } => {
                    let complete = screened.items.len().saturating_sub(1);
                    for item in screened.items.iter().take(complete) {
                        self.store_item(&keys, item.i, &item.translation, &item.note);
                    }
                }
                CoachEvent::Finished {
                    output: CoachOutput::Screen(screened),
                    ..
                } => {
                    for item in &screened.items {
                        self.store_item(&keys, item.i, &item.translation, &item.note);
                    }
                    self.memory.clear_pending(&keys);
                    self.in_flight.remove(&id);
                }
                CoachEvent::Finished { .. } => {
                    self.memory.clear_pending(&keys);
                    self.in_flight.remove(&id);
                }
                CoachEvent::Failed { message, .. } => {
                    tracing::warn!("屏幕阅读翻译失败：{message}");
                    self.memory.clear_pending(&keys);
                    self.in_flight.remove(&id);
                    self.give_up_on_repeat_failures(&keys);
                    self.last_error = Some(message);
                    self.retry_after = Instant::now() + RETRY_DELAY;
                }
                _ => {}
            }
        }
    }

    /// 这一批失败了：每块记一次，满两次的块标成「已跳过」不再发，卡片上能看到原因。
    fn give_up_on_repeat_failures(&mut self, keys: &[String]) {
        for key in keys {
            let count = self.failures.entry(key.clone()).or_insert(0);
            *count += 1;
            if *count >= MAX_FAILURES && self.memory.analysis(key).is_none() {
                tracing::warn!("屏幕阅读：这一块连续翻译失败，已跳过");
                self.memory.store(
                    key,
                    Analysis {
                        translation: "这一条翻译失败，已跳过".to_owned(),
                        note: "后端连续没有给出可用的回复".to_owned(),
                    },
                );
                for block in self.blocks.iter_mut().filter(|block| block.key == *key) {
                    block.analysis = self.memory.analysis(key).cloned();
                    block.pending = false;
                }
            }
        }
    }

    /// 预解码请求的收尾：成功就留下完整解读，失败就记下不再试。
    fn finish_warm(&mut self, id: u64, key: &str, event: CoachEvent) {
        match event {
            CoachEvent::Finished {
                output: output @ CoachOutput::Decode(_),
                ..
            } => {
                if self.decoded.len() >= DECODED_LIMIT {
                    self.decoded.clear();
                }
                self.decoded.insert(key.to_owned(), output);
                self.warming.remove(&id);
            }
            CoachEvent::Finished { .. } => {
                self.warm_failed.insert(key.to_owned());
                self.warming.remove(&id);
            }
            CoachEvent::Failed { message, .. } => {
                tracing::debug!("预解码失败：{message}");
                self.warm_failed.insert(key.to_owned());
                self.warming.remove(&id);
            }
            _ => {}
        }
    }

    fn store_item(&mut self, keys: &[String], index: usize, translation: &str, note: &str) {
        let Some(key) = index.checked_sub(1).and_then(|i| keys.get(i)) else {
            return;
        };
        if translation.trim().is_empty() {
            return;
        }
        if self.memory.analysis(key).is_none() {
            self.translated += 1;
        }
        self.memory.store(
            key,
            Analysis {
                translation: translation.trim().to_owned(),
                note: note.trim().to_owned(),
            },
        );
        for block in self.blocks.iter_mut().filter(|block| block.key == *key) {
            block.analysis = self.memory.analysis(key).cloned();
            block.pending = false;
        }
    }
}
