//! 结果缓存：同一段文本在同样的上下文里再来一次，不必再问后端（也不再花额度）。

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};

use crate::{CoachOutput, CoachRequest};

/// 缓存条数。
const CAPACITY: usize = 32;

#[derive(Default)]
pub struct ResultCache {
    entries: HashMap<u64, CoachOutput>,

    /// 插入顺序，满了丢最老的。
    order: VecDeque<u64>,
}

impl ResultCache {
    pub fn key(request: &CoachRequest) -> u64 {
        let mut hasher = DefaultHasher::new();
        (request.mode, &request.text, &request.context).hash(&mut hasher);
        hasher.finish()
    }

    pub fn get(&self, key: u64) -> Option<&CoachOutput> {
        self.entries.get(&key)
    }

    pub fn insert(&mut self, key: u64, output: CoachOutput) {
        if self.entries.insert(key, output).is_none() {
            self.order.push_back(key);
        }
        while self.order.len() > CAPACITY {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CoachContext, Decoded, Mode};

    fn request(id: u64, text: &str) -> CoachRequest {
        CoachRequest {
            id,
            mode: Mode::Decode,
            text: text.to_owned(),
            context: CoachContext::default(),
        }
    }

    #[test]
    fn key_ignores_the_request_id_but_not_the_text() {
        assert_eq!(
            ResultCache::key(&request(1, "a b c")),
            ResultCache::key(&request(2, "a b c"))
        );
        assert_ne!(
            ResultCache::key(&request(1, "a b c")),
            ResultCache::key(&request(1, "a b d"))
        );
    }

    #[test]
    fn evicts_the_oldest_when_full() {
        let mut cache = ResultCache::default();
        let output = CoachOutput::Decode(Decoded::default());
        for key in 0..(CAPACITY as u64 + 3) {
            cache.insert(key, output.clone());
        }
        assert!(cache.get(0).is_none() && cache.get(2).is_none());
        assert!(cache.get(3).is_some() && cache.get(CAPACITY as u64 + 2).is_some());
    }
}
