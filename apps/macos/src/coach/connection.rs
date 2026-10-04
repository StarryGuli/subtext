//! 设置页「测试连接」：在后台线程发一条最小请求，结果交给轮询取走。

use std::sync::mpsc::{Receiver, channel};

use subtext_coach::{CoachConfig, friendly, test_connection};

use super::Coach;

impl Coach {
    /// 开始测试。上一次的还没出结果就不重复发。
    pub fn start_test(&mut self, config: &CoachConfig) {
        if self.test.is_some() {
            return;
        }
        let (sender, receiver): (_, Receiver<String>) = channel();
        let config = config.clone();
        std::thread::spawn(move || {
            let message = match test_connection(&config) {
                Ok(report) => format!(
                    "双语教练连接正常：{} 在 {:.1} 秒内回复",
                    report.backend,
                    report.elapsed_ms as f64 / 1000.0
                ),
                Err(error) => format!("双语教练连接失败：{}", friendly(&error)),
            };
            let _ = sender.send(message);
        });
        self.test = Some(receiver);
        self.sync_monitor();
    }

    /// 测试结果到了就取走；没到返回 `None`。
    pub fn take_test_result(&mut self) -> Option<String> {
        let message = self.test.as_ref()?.try_recv().ok()?;
        self.test = None;
        self.sync_monitor();
        Some(message)
    }

    /// 密钥之类不在 [`CoachConfig`] 里的输入变了：丢掉旧服务重建，让后端读到新值。
    pub fn restart(&mut self, config: &CoachConfig) {
        self.service = config
            .enabled
            .then(|| subtext_coach::CoachService::start_with_cache(config, self.cache.clone()));
        self.sync_monitor();
    }
}
