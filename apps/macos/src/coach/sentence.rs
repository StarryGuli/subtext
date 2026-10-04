//! 从光标前的文字里取出刚写完的最后一句英文。纯逻辑，不碰 AppKit。

/// 句末标点；换行也当作一句的边界。
const TERMINATORS: &[char] = &['.', '!', '?', '。', '！', '？', '\n'];

/// 返回最后一句（含句末标点）和它在 `before` 里起点的 UTF-16 偏移。
///
/// 光标前有句末标点又跟着空格，说明那一句已经写完，取的就是那一句；没有标点就取从上一个标点后到光标的整段。
/// 空串或只有空白返回 `None`。
pub fn last_sentence(before: &str) -> Option<(&str, usize)> {
    let trimmed = before.trim_end();
    if trimmed.is_empty() {
        return None;
    }
    // 最后一个字符若就是句末标点，它属于这一句，找「再前一个」标点
    let search_end = trimmed
        .char_indices()
        .next_back()
        .filter(|(_, c)| TERMINATORS.contains(c))
        .map_or(trimmed.len(), |(index, _)| index);
    let start = trimmed[..search_end].rfind(TERMINATORS).map_or(0, |index| {
        index + trimmed[index..].chars().next().map_or(1, char::len_utf8)
    });
    let sentence = trimmed[start..].trim_start();
    if sentence.is_empty() {
        return None;
    }
    let sentence_start = trimmed.len() - sentence.len();
    let utf16_start = before[..sentence_start].encode_utf16().count();
    Some((sentence, utf16_start))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_the_unfinished_sentence_after_the_last_period() {
        let (sentence, offset) =
            last_sentence("Thanks for the PR. i dont get why it keep resetting").unwrap();
        assert_eq!(sentence, "i dont get why it keep resetting");
        assert_eq!(offset, "Thanks for the PR. ".encode_utf16().count());
    }

    #[test]
    fn a_finished_sentence_keeps_its_terminator() {
        let (sentence, _) = last_sentence("ok. can u check my wiring?  ").unwrap();
        assert_eq!(sentence, "can u check my wiring?");
    }

    #[test]
    fn newlines_and_full_width_marks_are_boundaries() {
        assert_eq!(
            last_sentence("first line\nsecond line here").unwrap().0,
            "second line here"
        );
        assert_eq!(
            last_sentence("你好。hello there friend").unwrap().0,
            "hello there friend"
        );
    }

    #[test]
    fn offsets_count_utf16_units_not_bytes() {
        let (sentence, offset) = last_sentence("好的 😀 sure. see you at the lab").unwrap();
        assert_eq!(sentence, "see you at the lab");
        assert_eq!(offset, "好的 😀 sure. ".encode_utf16().count());
    }

    #[test]
    fn nothing_to_take() {
        assert_eq!(last_sentence(""), None);
        assert_eq!(last_sentence("   \n "), None);
    }
}
