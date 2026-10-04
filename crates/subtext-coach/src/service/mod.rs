//! 教练服务：主线程只往通道里丢请求、从通道里取事件，永远不阻塞输入。

mod cache;
mod connection;
mod event;
mod worker;

use std::sync::mpsc::{Receiver, Sender, channel};

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
        let (request_sender, request_receiver) = channel();
        let (event_sender, event_receiver) = channel();
        let worker = worker::Worker::new(
            request_receiver,
            event_sender,
            backend::build(config),
            config.profile.clone(),
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
    fn bad_replies_become_friendly_failures() {
        let (service, _calls) = service_with("sorry I cannot help", Duration::ZERO);
        service
            .submit(request(1, "can you take a look today"))
            .unwrap();
        let events = wait_for(&service, |events| {
            events
                .iter()
                .any(|e| matches!(e, CoachEvent::Failed { .. }))
        });
        let Some(CoachEvent::Failed { message, .. }) = events.last() else {
            panic!("expected failure");
        };
        // 模型没按格式回、直接说了句话：把原话给用户看，而不是一句笼统的「再试一次」
        assert!(message.contains("sorry I cannot help"));
        let _ = CoachOutput::Decode(Default::default());
    }
}
