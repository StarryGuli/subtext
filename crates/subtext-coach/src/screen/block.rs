//! 把 OCR 出来的一行行文字合并成「消息块」：聊天里一条消息折成好几行，鼠标指着其中一行，要解释的是整条消息。

use super::Rect;

/// OCR 的一行。
#[derive(Debug, Clone, PartialEq)]
pub struct OcrLine {
    pub text: String,

    pub rect: Rect,
}

/// 合并后的一块（一条消息 / 一个段落）。
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub text: String,

    pub rect: Rect,

    /// 由几行合成。
    pub lines: usize,
}

/// 上下两行的间距小于行高的这个倍数，才可能是同一块。
const MAX_GAP_RATIO: f32 = 0.45;

/// 两行左边缘相差不超过行高的这个倍数，才算左对齐的同一栏（聊天气泡里的折行、段落的续行）。
const MAX_LEFT_SHIFT_RATIO: f32 = 2.2;

/// 把行合并成块，按从上到下、从左到右排好。
///
/// 不按阅读顺序一行行往下接：聊天窗口左边是联系人列表、右边是对话，按 y 排序会把两栏交错。
/// 改成两两比较，满足「上下相邻且左对齐」的行用并查集并到一起，各栏互不干扰。
pub fn group_blocks(lines: &[OcrLine]) -> Vec<Block> {
    let lines: Vec<&OcrLine> = lines
        .iter()
        .filter(|line| !line.text.trim().is_empty() && line.rect.height > 0.0)
        .collect();
    let mut parent: Vec<usize> = (0..lines.len()).collect();
    for i in 0..lines.len() {
        for j in 0..lines.len() {
            if i != j && continues(&lines[i].rect, &lines[j].rect) {
                union(&mut parent, i, j);
            }
        }
    }
    let mut groups: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    for index in 0..lines.len() {
        let root = find(&mut parent, index);
        groups.entry(root).or_default().push(index);
    }
    let mut blocks: Vec<Block> = groups
        .into_values()
        .map(|mut members| {
            members.sort_by(|a, b| lines[*a].rect.y.total_cmp(&lines[*b].rect.y));
            let rect = members
                .iter()
                .map(|index| lines[*index].rect)
                .reduce(|a, b| a.union(&b))
                .unwrap_or_default();
            let text = join_lines(members.iter().map(|index| lines[*index].text.trim()));
            Block {
                text,
                rect,
                lines: members.len(),
            }
        })
        .collect();
    blocks.sort_by(|a, b| {
        a.rect
            .y
            .total_cmp(&b.rect.y)
            .then(a.rect.x.total_cmp(&b.rect.x))
    });
    blocks
}

/// `lower` 是不是紧接在 `upper` 下面的续行。
fn continues(upper: &Rect, lower: &Rect) -> bool {
    if lower.y <= upper.y {
        return false;
    }
    let height = upper.height.min(lower.height);
    let gap = lower.y - upper.bottom();
    // 上下行有一点重叠（OCR 框不准）也算相邻
    gap < height * MAX_GAP_RATIO
        && gap > -height
        && (lower.x - upper.x).abs() < height * MAX_LEFT_SHIFT_RATIO
}

fn find(parent: &mut [usize], mut index: usize) -> usize {
    while parent[index] != index {
        parent[index] = parent[parent[index]];
        index = parent[index];
    }
    index
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let (a, b) = (find(parent, a), find(parent, b));
    if a != b {
        parent[a.max(b)] = a.min(b);
    }
}

/// 折行拼回一句：英文行之间补空格，两边都是汉字就直接连。
fn join_lines<'a>(lines: impl Iterator<Item = &'a str>) -> String {
    let mut out = String::new();
    for line in lines {
        if let (Some(last), Some(first)) = (out.chars().last(), line.chars().next()) {
            let cjk = |c: char| matches!(c as u32, 0x3000..=0x9FFF | 0xFF00..=0xFFEF);
            if !(last.is_whitespace() || cjk(last) && cjk(first)) {
                out.push(' ');
            }
        }
        out.push_str(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, x: f32, y: f32, width: f32) -> OcrLine {
        OcrLine {
            text: text.to_owned(),
            rect: Rect::new(x, y, width, 20.0),
        }
    }

    #[test]
    fn wrapped_message_becomes_one_block() {
        let blocks = group_blocks(&[
            line("hey, no rush at all but did you get", 100.0, 100.0, 300.0),
            line("a chance to look at that PR?", 100.0, 124.0, 220.0),
        ]);
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0].text,
            "hey, no rush at all but did you get a chance to look at that PR?"
        );
        assert_eq!(blocks[0].lines, 2);
        assert_eq!(blocks[0].rect.height, 44.0);
    }

    #[test]
    fn separate_messages_with_a_gap_stay_separate() {
        let blocks = group_blocks(&[
            line("first message here", 100.0, 100.0, 200.0),
            line("second message later", 100.0, 160.0, 200.0),
        ]);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].text, "first message here");
    }

    #[test]
    fn two_columns_interleaved_in_y_do_not_mix() {
        // 左栏联系人列表、右栏对话，行在 y 上交错
        let blocks = group_blocks(&[
            line("Alice", 10.0, 100.0, 60.0),
            line("see you at the lab tomorrow", 400.0, 102.0, 260.0),
            line("Bob", 10.0, 160.0, 40.0),
            line("ok sounds good", 400.0, 162.0, 140.0),
        ]);
        let texts: Vec<&str> = blocks.iter().map(|b| b.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Alice",
                "see you at the lab tomorrow",
                "Bob",
                "ok sounds good"
            ]
        );
    }

    #[test]
    fn right_aligned_bubble_lines_of_my_own_message_still_group() {
        let blocks = group_blocks(&[
            line("can you check my wiring", 520.0, 300.0, 180.0),
            line("when you get a chance?", 540.0, 324.0, 160.0),
        ]);
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn chinese_lines_join_without_spaces_and_blank_lines_are_dropped() {
        let blocks = group_blocks(&[
            line("你好，今天下午有空", 100.0, 100.0, 200.0),
            line("   ", 100.0, 124.0, 10.0),
            line("一起去实验室吗", 100.0, 126.0, 150.0),
        ]);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].text, "你好，今天下午有空一起去实验室吗");
    }

    #[test]
    fn blocks_come_out_top_to_bottom() {
        let blocks = group_blocks(&[
            line("bottom", 100.0, 400.0, 60.0),
            line("top", 100.0, 100.0, 40.0),
        ]);
        assert_eq!(blocks[0].text, "top");
        assert_eq!(blocks[1].text, "bottom");
    }
}
