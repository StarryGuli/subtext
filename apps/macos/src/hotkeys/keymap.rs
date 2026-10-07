//! 配置里的键位 → Carbon 热键的虚拟键码与修饰键位。

use subtext_platform::{KeyCombo, Modifiers};

const COMMAND: u32 = 1 << 8;
const SHIFT: u32 = 1 << 9;
const OPTION: u32 = 1 << 11;
const CONTROL: u32 = 1 << 12;

/// ANSI 键盘上字母与数字键的虚拟键码。
fn key_code(key: char) -> Option<u32> {
    Some(match key {
        'a' => 0,
        's' => 1,
        'd' => 2,
        'f' => 3,
        'h' => 4,
        'g' => 5,
        'z' => 6,
        'x' => 7,
        'c' => 8,
        'v' => 9,
        'b' => 11,
        'q' => 12,
        'w' => 13,
        'e' => 14,
        'r' => 15,
        'y' => 16,
        't' => 17,
        '1' => 18,
        '2' => 19,
        '3' => 20,
        '4' => 21,
        '6' => 22,
        '5' => 23,
        '9' => 25,
        '7' => 26,
        '8' => 28,
        '0' => 29,
        'o' => 31,
        'u' => 32,
        'i' => 34,
        'p' => 35,
        'l' => 37,
        'j' => 38,
        'k' => 40,
        'n' => 45,
        'm' => 46,
        _ => return None,
    })
}

fn modifier_bits(modifiers: Modifiers) -> u32 {
    let mut bits = 0;
    if modifiers.command {
        bits |= COMMAND;
    }
    if modifiers.shift {
        bits |= SHIFT;
    }
    if modifiers.option {
        bits |= OPTION;
    }
    if modifiers.control {
        bits |= CONTROL;
    }
    bits
}

/// 注册热键要的（键码，修饰键位）；键认不出，或没有 ⌘ / ⌃ / ⌥（只有 ⇧ 会把普通大写字母整个吞掉）返回 `None`。
pub(super) fn carbon(combo: KeyCombo) -> Option<(u32, u32)> {
    let m = combo.modifiers;
    if !(m.command || m.control || m.option) {
        return None;
    }
    Some((key_code(combo.key)?, modifier_bits(m)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combo(text: &str) -> KeyCombo {
        text.parse().unwrap()
    }

    #[test]
    fn command_shift_d_maps_to_carbon_bits() {
        assert_eq!(carbon(combo("command+shift+d")), Some((2, COMMAND | SHIFT)));
        assert_eq!(
            carbon(combo("control+option+r")),
            Some((15, CONTROL | OPTION))
        );
    }

    #[test]
    fn shift_only_combos_are_refused() {
        assert_eq!(carbon(combo("shift+d")), None);
    }

    #[test]
    fn every_letter_and_digit_has_a_key_code() {
        for key in ('a'..='z').chain('0'..='9') {
            assert!(key_code(key).is_some(), "{key}");
        }
    }
}
