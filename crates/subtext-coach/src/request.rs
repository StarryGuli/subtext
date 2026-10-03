//! 发给教练的一次请求：文本、模式，以及决定语气与缩写的上下文。

use crate::Mode;

/// 请求的上下文。字段都是可选的线索，缺了照样能答。
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct CoachContext {
    /// 当前应用的名字或 bundle id，用来推断场合（邮件偏正式，聊天偏随意）。
    pub app: Option<String>,

    /// 光标前最近输入的文字，帮助判断话题与语气。
    pub before: String,

    /// 对方最近发来、刚解码过的那条消息；写回复时按它的语气接。
    pub peer_message: Option<String>,
}

/// 一次教练请求。`id` 单调递增，面板只认最新的。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CoachRequest {
    pub id: u64,

    pub mode: Mode,

    /// 要解码 / 翻译 / 修改的原文。
    pub text: String,

    pub context: CoachContext,
}
