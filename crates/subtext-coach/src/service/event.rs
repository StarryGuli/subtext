//! 后台线程回给界面的事件。

use crate::{CoachOutput, Mode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoachEvent {
    /// 请求已开始处理；界面可以显示「思考中」。
    Started { id: u64, mode: Mode },

    /// 成功。`cached` 为真表示命中缓存、没有再问后端。
    Finished {
        id: u64,
        output: CoachOutput,
        cached: bool,
    },

    /// 失败，`message` 是给人看的原因。
    Failed {
        id: u64,
        mode: Mode,
        message: String,
    },
}

impl CoachEvent {
    pub fn id(&self) -> u64 {
        match self {
            Self::Started { id, .. } | Self::Finished { id, .. } | Self::Failed { id, .. } => *id,
        }
    }
}
