//! 双语教练：解码收到的英文、把中文组成地道英文、修改英文草稿。
//!
//! 平台无关。壳负责触发（复制、上屏）与展示；这里负责闸门（什么能外发）、提示词、后端与后台线程。
//! 后端可以是本机 Claude Code / Codex 命令行，也可以是 OpenAI 兼容接口或 Anthropic API。
//! 设计见 `docs/design/coach.md`。

pub mod backend;
mod config;
mod error;
pub mod gate;
mod memory;
mod mode;
mod output;
pub mod prompt;
mod request;
mod service;

pub use config::{BackendKind, CoachConfig};
pub use error::CoachError;
pub use memory::{ConversationMemory, PEER_TTL};
pub use mode::{Mode, Trigger};
pub use output::{
    Alternative, CoachOutput, ComposeOption, ComposePoint, Composed, DecodePoint, Decoded, Edited,
    Fix, Tone,
};
pub use request::{CoachContext, CoachRequest};
pub use service::{CoachEvent, CoachService, ConnectionReport, friendly, test_connection};
