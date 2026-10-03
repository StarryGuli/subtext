//! 连接测试：设置页的「测试连接」按钮用，发一个最小请求看后端通不通、多慢。

use std::time::Instant;

use crate::{CoachConfig, CoachError, backend};

/// 测试结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionReport {
    /// 实际连到的后端描述，如 `Claude Code · haiku · /Users/…/claude`。
    pub backend: String,

    pub elapsed_ms: u128,
}

/// 同步测试，会阻塞：调用方要放到后台线程里。
pub fn test_connection(config: &CoachConfig) -> Result<ConnectionReport, CoachError> {
    let backend = backend::build(config);
    let started = Instant::now();
    let reply = backend.complete("Reply with exactly the word OK and nothing else.", "ping")?;
    if reply.trim().is_empty() {
        return Err(CoachError::EmptyReply);
    }
    Ok(ConnectionReport {
        backend: backend.describe(),
        elapsed_ms: started.elapsed().as_millis(),
    })
}
