//! 后端：把「系统提示 + 用户消息」变成一段文本回复。四种实现，行为一致。

mod anthropic;
mod binary;
mod claude_cli;
mod codex_cli;
mod http;
mod openai;
mod process;

pub use anthropic::Anthropic;
pub use claude_cli::ClaudeCli;
pub use codex_cli::CodexCli;
pub use openai::OpenAi;

use crate::{BackendKind, CoachConfig, CoachError};

/// 一次问答。实现必须在 `timeout` 内返回，且不留后台进程。
pub trait Backend: Send {
    fn complete(&self, system: &str, user: &str) -> Result<String, CoachError>;

    /// 流式：生成过程中每来一段新文本就调一次 `on_text`（只给增量），返回完整文本。
    /// 不支持流式的后端退化成「等完整回复，一次性给出」。
    fn stream(
        &self,
        system: &str,
        user: &str,
        on_text: &mut dyn FnMut(&str),
    ) -> Result<String, CoachError> {
        let reply = self.complete(system, user)?;
        on_text(&reply);
        Ok(reply)
    }

    /// 设置页里显示的简短描述。
    fn describe(&self) -> String;
}

/// 按配置造后端。
pub fn build(config: &CoachConfig) -> Box<dyn Backend> {
    match config.backend {
        BackendKind::ClaudeCli => Box::new(ClaudeCli::new(config)),
        BackendKind::CodexCli => Box::new(CodexCli::new(config)),
        BackendKind::OpenAi => Box::new(OpenAi::new(config)),
        BackendKind::Anthropic => Box::new(Anthropic::new(config)),
    }
}
