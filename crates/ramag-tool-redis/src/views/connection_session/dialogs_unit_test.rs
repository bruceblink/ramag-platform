use super::truncate_for_dialog;

#[test]
fn short_string_unchanged() {
    assert_eq!(truncate_for_dialog("abc", 10), "abc");
}

#[test]
fn exact_length_unchanged() {
    assert_eq!(truncate_for_dialog("abcde", 5), "abcde");
}

#[test]
fn over_length_truncated() {
    assert_eq!(truncate_for_dialog("abcdef", 3), "abc…");
}

#[test]
fn utf8_chinese_safe() {
    assert_eq!(truncate_for_dialog("你好世界呀", 3), "你好世…");
}

#[test]
fn utf8_emoji_safe() {
    let result = truncate_for_dialog("ab😀cd", 3);
    assert_eq!(result, "ab😀…");
}

#[test]
fn empty_string() {
    assert_eq!(truncate_for_dialog("", 5), "");
}

#[test]
fn control_characters_are_safe_for_single_line_dialogs() {
    assert_eq!(truncate_for_dialog("a\nb\0c", 10), "a b�c");
}
