//! 教练服务：主线程只往通道里丢请求、从通道里取事件，永远不阻塞输入。

mod cache;
mod connection;
mod event;
mod worker;

use std::sync::mpsc::{Receiver, Sender, channel};

pub use cache::SharedCache;
pub use connection::{ConnectionReport, test_connection};
pub use event::CoachEvent;
pub use worker::friendly;

use crate::gate::Gate;
use crate::{CoachConfig, CoachError, CoachRequest, backend};

pub struct CoachService {
    requests: Sender<CoachRequest>,

    events: Receiver<CoachEvent>,

    gate: Gate,
}

impl CoachService {
    /// 按配置起后台线程。配置变了就丢掉旧服务、造新的：旧线程在发送端关闭后自己退出。
    pub fn start(config: &CoachConfig) -> Self {
        Self::start_with_cache(config, SharedCache::in_memory())
    }

    /// 同 [`Self::start`]，但缓存由调用方给：解码与屏幕阅读共用一份，换后端、开关教练重建服务时历史也不丢。
    pub fn start_with_cache(config: &CoachConfig, cache: SharedCache) -> Self {
        Self::start_with(config, cache, true)
    }

    /// 屏幕阅读用：积压的请求一个都不丢，按顺序全部处理。
    pub fn start_queued(config: &CoachConfig, cache: SharedCache) -> Self {
        Self::start_with(config, cache, false)
    }

    fn start_with(config: &CoachConfig, cache: SharedCache, latest_only: bool) -> Self {
        let (request_sender, request_receiver) = channel();
        let (event_sender, event_receiver) = channel();
        let worker = worker::Worker::new(
            request_receiver,
            event_sender,
            backend::build(config),
            config.profile.clone(),
            cache,
            latest_only,
        );
        std::thread::Builder::new()
            .name("subtext-coach".to_owned())
            .spawn(move || worker.run())
            .expect("spawn coach worker");
        Self {
            requests: request_sender,
            events: event_receiver,
            gate: Gate::new(config.max_chars, config.skip_apps.clone()),
        }
    }

    pub fn gate(&self) -> &Gate {
        &self.gate
    }

    /// 提交请求；新请求会让还没开始处理的旧请求作废。
    pub fn submit(&self, request: CoachRequest) -> Result<(), CoachError> {
        self.requests
            .send(request)
            .map_err(|_| CoachError::WorkerStopped)
    }

    /// 取走已到达的事件，不阻塞。
    pub fn poll(&self) -> Vec<CoachEvent> {
        self.events.try_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::{CoachContext, CoachOutput, Mode};

    /// 假后端：按预设回复，并记录收到的请求次数。
    struct Fake {
        reply: String,

        delay: Duration,

        calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl backend::Backend for Fake {
        fn complete(&self, _system: &str, _user: &str) -> Result<String, CoachError> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            std::thread::sleep(self.delay);
            Ok(self.reply.clone())
        }

        fn describe(&self) -> String {
            "fake".to_owned()
        }
    }

    fn service_with(
        reply: &str,
        delay: Duration,
    ) -> (CoachService, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        let (request_sender, request_receiver) = channel();
        let (event_sender, event_receiver) = channel();
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let fake = Fake {
            reply: reply.to_owned(),
            delay,
            calls: calls.clone(),
        };
        let worker = worker::Worker::new(
            request_receiver,
            event_sender,
            Box::new(fake),
            "profile".to_owned(),
            SharedCache::in_memory(),
            true,
        );
        std::thread::spawn(move || worker.run());
        (
            CoachService {
                requests: request_sender,
                events: event_receiver,
                gate: Gate::new(1500, Vec::new()),
            },
            calls,
        )
    }

    fn request(id: u64, text: &str) -> CoachRequest {
        CoachRequest {
            id,
            mode: Mode::Decode,
            text: text.to_owned(),
            context: CoachContext::default(),
        }
    }

    fn wait_for(service: &CoachService, done: impl Fn(&[CoachEvent]) -> bool) -> Vec<CoachEvent> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut seen = Vec::new();
        while Instant::now() < deadline {
            seen.extend(service.poll());
            if done(&seen) {
                return seen;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out; saw {seen:?}");
    }

    const REPLY: &str = r#"{"situation": "同事在催你", "points": []}"#;

    /// 一个字一个字吐出回复的后端，用来验证流式：中间应有越来越完整的 Partial，最后是 Finished。
    struct Trickle;

    impl backend::Backend for Trickle {
        fn complete(&self, _system: &str, _user: &str) -> Result<String, CoachError> {
            unreachable!("worker 应该走 stream")
        }

        fn stream(
            &self,
            _system: &str,
            _user: &str,
            on_text: &mut dyn FnMut(&str),
        ) -> Result<String, CoachError> {
            let reply = r#"{"situation": "同事在催你，但催得很客气", "points": []}"#;
            for ch in reply.chars() {
                on_text(&ch.to_string());
                std::thread::sleep(Duration::from_millis(15));
            }
            Ok(reply.to_owned())
        }

        fn describe(&self) -> String {
            "trickle".to_owned()
        }
    }

    #[test]
    fn partial_results_grow_before_the_final_one() {
        let (request_sender, request_receiver) = channel();
        let (event_sender, event_receiver) = channel();
        let worker = worker::Worker::new(
            request_receiver,
            event_sender,
            Box::new(Trickle),
            "profile".to_owned(),
            SharedCache::in_memory(),
            true,
        );
        std::thread::spawn(move || worker.run());
        let service = CoachService {
            requests: request_sender,
            events: event_receiver,
            gate: Gate::new(1500, Vec::new()),
        };
        service
            .submit(request(1, "can you take a look today"))
            .unwrap();
        let events = wait_for(&service, |events| {
            events
                .iter()
                .any(|e| matches!(e, CoachEvent::Finished { .. }))
        });
        let lengths: Vec<usize> = events
            .iter()
            .filter_map(|event| match event {
                CoachEvent::Partial {
                    output: CoachOutput::Decode(decoded),
                    ..
                } => Some(decoded.situation.chars().count()),
                _ => None,
            })
            .collect();
        assert!(lengths.len() >= 2, "应该有多次中间结果: {lengths:?}");
        assert!(
            lengths.windows(2).all(|pair| pair[0] < pair[1]),
            "{lengths:?}"
        );
        assert!(matches!(events.last(), Some(CoachEvent::Finished { .. })));
    }

    #[test]
    fn answers_and_then_serves_repeats_from_cache() {
        let (service, calls) = service_with(REPLY, Duration::ZERO);
        service
            .submit(request(1, "lmk if you want me to merge it"))
            .unwrap();
        let first = wait_for(&service, |events| {
            events
                .iter()
                .any(|e| matches!(e, CoachEvent::Finished { .. }))
        });
        assert!(matches!(
            first.first(),
            Some(CoachEvent::Started { id: 1, .. })
        ));
        service
            .submit(request(2, "lmk if you want me to merge it"))
            .unwrap();
        let second = wait_for(&service, |events| !events.is_empty());
        assert!(matches!(
            &second[0],
            CoachEvent::Finished {
                id: 2,
                cached: true,
                ..
            }
        ));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn backlog_keeps_only_the_latest_request() {
        let (service, calls) = service_with(REPLY, Duration::from_millis(150));
        service.submit(request(1, "first message here ok")).unwrap();
        wait_for(&service, |events| {
            events
                .iter()
                .any(|e| matches!(e, CoachEvent::Started { id: 1, .. }))
        });
        // 第一个还在处理时连发三个：只有最后一个会被处理
        for (id, text) in [
            (2, "second message here ok"),
            (3, "third message here ok"),
            (4, "fourth message here ok"),
        ] {
            service.submit(request(id, text)).unwrap();
        }
        let events = wait_for(&service, |events| {
            events
                .iter()
                .any(|e| matches!(e, CoachEvent::Finished { id: 4, .. }))
        });
        assert!(!events.iter().any(|e| e.id() == 2 || e.id() == 3));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn prose_replies_are_shown_as_plain_text_and_not_cached() {
        let (service, calls) = service_with("sorry I cannot help with that one", Duration::ZERO);
        service
            .submit(request(1, "can you take a look today"))
            .unwrap();
        let events = wait_for(&service, |events| {
            events
                .iter()
                .any(|e| matches!(e, CoachEvent::Finished { .. }))
        });
        // 模型没按格式回、直接说了句话：原文给用户看，而不是报错
        let Some(CoachEvent::Finished { output, cached, .. }) = events.last() else {
            panic!("expected a finished event");
        };
        assert!(!cached);
        assert!(matches!(output, CoachOutput::Plain { text, .. } if text.contains("cannot help")));
        // 纯文本不进缓存：同一段文字再来一次要重新问，说不定这次就按格式回了
        service
            .submit(request(2, "can you take a look today"))
            .unwrap();
        wait_for(&service, |events| {
            events
                .iter()
                .any(|e| matches!(e, CoachEvent::Finished { id: 2, .. }))
        });
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn a_shared_cache_serves_a_second_service_with_no_backend_call() {
        let cache = SharedCache::in_memory();
        let make = |reply: &str| {
            let (request_sender, request_receiver) = channel();
            let (event_sender, event_receiver) = channel();
            let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let fake = Fake {
                reply: reply.to_owned(),
                delay: Duration::ZERO,
                calls: calls.clone(),
            };
            let worker = worker::Worker::new(
                request_receiver,
                event_sender,
                Box::new(fake),
                "profile".to_owned(),
                cache.clone(),
                true,
            );
            std::thread::spawn(move || worker.run());
            (
                CoachService {
                    requests: request_sender,
                    events: event_receiver,
                    gate: Gate::new(1500, Vec::new()),
                },
                calls,
            )
        };
        let (first, first_calls) = make(REPLY);
        first
            .submit(request(1, "lmk if you want me to merge it"))
            .unwrap();
        wait_for(&first, |events| {
            events
                .iter()
                .any(|e| matches!(e, CoachEvent::Finished { .. }))
        });
        assert_eq!(first_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        // 换一个服务（比如换了后端、开关了教练），同一段文字（标点不同、不同应用）照样命中
        let (second, second_calls) = make("{}");
        let mut other = request(5, "LMK, if you want me to merge it!");
        other.context.app = Some("com.apple.Safari".to_owned());
        second.submit(other).unwrap();
        let events = wait_for(&second, |events| {
            events
                .iter()
                .any(|e| matches!(e, CoachEvent::Finished { .. }))
        });
        assert!(matches!(
            events.last(),
            Some(CoachEvent::Finished { cached: true, .. })
        ));
        assert_eq!(second_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn queued_mode_answers_every_request_instead_of_keeping_only_the_latest() {
        let (request_sender, request_receiver) = channel();
        let (event_sender, event_receiver) = channel();
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let fake = Fake {
            reply: REPLY.to_owned(),
            delay: Duration::from_millis(80),
            calls: calls.clone(),
        };
        let worker = worker::Worker::new(
            request_receiver,
            event_sender,
            Box::new(fake),
            "profile".to_owned(),
            SharedCache::in_memory(),
            false,
        );
        std::thread::spawn(move || worker.run());
        let service = CoachService {
            requests: request_sender,
            events: event_receiver,
            gate: Gate::new(1500, Vec::new()),
        };
        for (id, text) in [
            (1, "first message here ok"),
            (2, "second message here ok"),
            (3, "third message here ok"),
        ] {
            service.submit(request(id, text)).unwrap();
        }
        let events = wait_for(&service, |events| {
            (1..=3).all(|id| {
                events
                    .iter()
                    .any(|e| matches!(e, CoachEvent::Finished { id: done, .. } if *done == id))
            })
        });
        assert!(!events.is_empty());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
    }
}
