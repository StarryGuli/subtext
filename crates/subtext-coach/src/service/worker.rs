//! 后台线程：收请求、只留最新的、查缓存、问后端、把结果送回去。

use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;

use super::cache::ResultCache;
use super::event::CoachEvent;
use crate::backend::Backend;
use crate::{CoachError, CoachOutput, CoachRequest, prompt};

pub struct Worker {
    requests: Receiver<CoachRequest>,

    events: Sender<CoachEvent>,

    backend: Box<dyn Backend>,

    profile: String,

    cache: ResultCache,
}

impl Worker {
    pub fn new(
        requests: Receiver<CoachRequest>,
        events: Sender<CoachEvent>,
        backend: Box<dyn Backend>,
        profile: String,
    ) -> Self {
        Self {
            requests,
            events,
            backend,
            profile,
            cache: ResultCache::default(),
        }
    }

    /// 阻塞运行，发送端全部关闭后返回。
    pub fn run(mut self) {
        while let Ok(first) = self.requests.recv() {
            let request = self.latest(first);
            if !self.handle(&request) {
                return;
            }
        }
    }

    /// 积压的请求只留最后一个：用户已经往下写了，前面的答案没人看。
    fn latest(&self, mut request: CoachRequest) -> CoachRequest {
        while let Ok(newer) = self.requests.try_recv() {
            request = newer;
        }
        request
    }

    /// 处理一个请求；界面那头没了返回 `false`。
    fn handle(&mut self, request: &CoachRequest) -> bool {
        let key = ResultCache::key(request);
        if let Some(output) = self.cache.get(key) {
            tracing::debug!(id = request.id, "教练命中缓存");
            return self.send(CoachEvent::Finished {
                id: request.id,
                output: output.clone(),
                cached: true,
            });
        }
        if !self.send(CoachEvent::Started {
            id: request.id,
            mode: request.mode,
        }) {
            return false;
        }
        let started = Instant::now();
        match self.ask(request) {
            Ok(output) => {
                tracing::info!(
                    id = request.id,
                    mode = ?request.mode,
                    elapsed_ms = started.elapsed().as_millis(),
                    backend = %self.backend.describe(),
                    "教练完成"
                );
                self.cache.insert(key, output.clone());
                self.send(CoachEvent::Finished {
                    id: request.id,
                    output,
                    cached: false,
                })
            }
            Err(error) => {
                tracing::warn!(id = request.id, mode = ?request.mode, %error, "教练失败");
                self.send(CoachEvent::Failed {
                    id: request.id,
                    mode: request.mode,
                    message: friendly(&error),
                })
            }
        }
    }

    fn ask(&self, request: &CoachRequest) -> Result<CoachOutput, CoachError> {
        let prompt = prompt::build(request, &self.profile);
        let reply = self.backend.complete(&prompt.system, &prompt.user)?;
        CoachOutput::parse(request.mode, &reply)
    }

    fn send(&self, event: CoachEvent) -> bool {
        self.events.send(event).is_ok()
    }
}

/// 面板里显示的失败原因：中文、说人话、告诉用户下一步。
pub fn friendly(error: &CoachError) -> String {
    match error {
        CoachError::BinaryNotFound(name) => {
            format!("没找到 {name} 命令。请先安装，或在教练设置里填它的路径。")
        }
        CoachError::MissingApiKey(env) => {
            format!("还没填 API 密钥。请在教练设置里填写，或设置环境变量 {env}。")
        }
        CoachError::Timeout(ms) => format!(
            "等了 {} 秒还没有回复，已放弃。可以在设置里调大超时。",
            ms / 1000
        ),
        CoachError::Api {
            status: 401 | 403, ..
        } => "密钥被拒绝（401/403），请检查密钥与接口地址。".to_owned(),
        CoachError::Api { status: 429, .. } => "请求太频繁或额度用完了（429）。".to_owned(),
        CoachError::Api { status, .. } => format!("接口返回错误 {status}。详情见日志。"),
        CoachError::BadReply(reply)
            if reply.chars().count() >= 4 && !reply.contains(['{', '}']) =>
        {
            // 模型没按格式回，而是直接说了句话（多半是嫌输入太零碎）：原话给用户看
            format!("教练说：{}", reply.trim())
        }
        CoachError::EmptyReply | CoachError::BadReply(_) => {
            "模型这次没有给出可用的回复，可以再试一次。".to_owned()
        }
        CoachError::CommandFailed {
            command, stderr, ..
        } if needs_login(stderr) => {
            let login = if command == "claude" {
                "claude auth login"
            } else {
                "codex login"
            };
            format!(
                "命令行 {command} 没有登录或登录过期了（桌面 App 里的登录和命令行是两套）。请在终端运行：{login}，授权后再试。"
            )
        }
        CoachError::CommandFailed { command, .. } => {
            format!("{command} 运行失败，可能额度用完了。详情见日志。")
        }
        other => format!("教练出错了：{other}"),
    }
}

/// 命令行工具的报错里有没有「没登录」的特征。
fn needs_login(stderr: &str) -> bool {
    let lowered = stderr.to_lowercase();
    [
        "authenticate",
        "login",
        "log in",
        "expired",
        "unauthorized",
        "api key",
    ]
    .iter()
    .any(|marker| lowered.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friendly_messages_tell_the_user_what_to_do() {
        assert!(friendly(&CoachError::BinaryNotFound("claude".into())).contains("安装"));
        assert!(friendly(&CoachError::Timeout(90_000)).contains("90 秒"));
        assert!(
            friendly(&CoachError::Api {
                status: 401,
                body: String::new()
            })
            .contains("密钥")
        );
        assert!(friendly(&CoachError::EmptyReply).contains("再试"));
        let expired = CoachError::CommandFailed {
            command: "claude".into(),
            status: "is_error".into(),
            stderr: "Failed to authenticate: OAuth session expired".into(),
        };
        assert!(friendly(&expired).contains("登录"));
    }
}
