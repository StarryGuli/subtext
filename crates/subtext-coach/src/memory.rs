//! 短时记忆：刚解码过的对方消息，写回复时用来接上对方的语气。

use std::time::{Duration, Instant};

/// 对方消息保留多久：超过就当对话已经换了话题。
pub const PEER_TTL: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Default)]
pub struct ConversationMemory {
    peer: Option<(String, Instant)>,
}

impl ConversationMemory {
    /// 记下刚解码的那条对方消息。
    pub fn remember_peer(&mut self, message: &str) {
        self.peer = Some((message.trim().to_owned(), Instant::now()));
    }

    /// 还没过期的对方消息。
    pub fn peer_message(&self) -> Option<&str> {
        self.peer_within(PEER_TTL)
    }

    fn peer_within(&self, ttl: Duration) -> Option<&str> {
        self.peer
            .as_ref()
            .filter(|(_, at)| at.elapsed() <= ttl)
            .map(|(message, _)| message.as_str())
    }

    pub fn clear(&mut self) {
        self.peer = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_then_expires() {
        let mut memory = ConversationMemory::default();
        assert_eq!(memory.peer_message(), None);
        memory.remember_peer("  can you send it by friday?  ");
        assert_eq!(memory.peer_message(), Some("can you send it by friday?"));
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(memory.peer_within(Duration::ZERO), None);
        memory.clear();
        assert_eq!(memory.peer_message(), None);
    }
}
