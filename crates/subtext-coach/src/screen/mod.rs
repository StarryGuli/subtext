//! 屏幕阅读的平台无关部分：OCR 行合并成消息块、记住哪些块分析过、鼠标指着哪一块。
//!
//! 截屏与 OCR 由壳做（macOS 用 `screencapture` 加 Vision），这里只吃它们给的文字与位置。

mod block;
mod memory;
mod rect;

pub use block::{Block, OcrLine, group_blocks};
pub use memory::{Analysis, ScreenBlock, ScreenMemory, block_at};
pub use rect::Rect;
