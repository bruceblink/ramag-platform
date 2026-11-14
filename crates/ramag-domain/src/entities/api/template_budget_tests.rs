use std::collections::BTreeMap;

use super::{append_template_segment, resolve_template};

const LABEL: &str = "URL";

fn variables() -> BTreeMap<String, String> {
    BTreeMap::from([("value".into(), "雪".into())])
}

#[test]
fn repeated_variables_are_bounded_before_appending_each_value() {
    let values = variables();

    assert_eq!(
        resolve_template("{{value}}{{value}}", &values, LABEL, 6),
        Ok("雪雪".into())
    );
    assert_eq!(
        resolve_template("{{value}}{{value}}", &values, LABEL, 5),
        Err("URL展开后超过 5 bytes 上限".into())
    );
}

#[test]
fn literals_and_replacements_respect_the_exact_byte_budget() {
    let values = variables();

    assert_eq!(
        resolve_template("ab{{value}}cd", &values, LABEL, 7),
        Ok("ab雪cd".into())
    );
    assert_eq!(
        resolve_template("ab{{value}}cd", &values, LABEL, 6),
        Err("URL展开后超过 6 bytes 上限".into())
    );
}

#[test]
fn trailing_literal_is_checked_before_it_is_appended() {
    let values = variables();

    assert_eq!(
        resolve_template("{{value}}tail", &values, LABEL, 6),
        Err("URL展开后超过 6 bytes 上限".into())
    );
}

#[test]
fn templates_without_variables_are_still_limited() {
    let values = variables();

    assert_eq!(
        resolve_template("plain", &values, LABEL, 5),
        Ok("plain".into())
    );
    assert_eq!(
        resolve_template("plain", &values, LABEL, 4),
        Err("URL展开后超过 4 bytes 上限".into())
    );
}

#[test]
fn utf8_values_are_budgeted_by_bytes() {
    let values = variables();

    assert_eq!(
        resolve_template("{{value}}", &values, LABEL, 3),
        Ok("雪".into())
    );
    assert_eq!(
        resolve_template("{{value}}", &values, LABEL, 2),
        Err("URL展开后超过 2 bytes 上限".into())
    );
}

#[test]
fn existing_missing_unclosed_and_invalid_name_errors_are_preserved() {
    let values = variables();

    assert_eq!(
        resolve_template("{{missing}}", &values, LABEL, 128),
        Err("缺少 API 环境变量：missing".into())
    );
    assert_eq!(
        resolve_template("{{unclosed", &values, LABEL, 128),
        Err("URL变量占位符未闭合".into())
    );
    assert_eq!(
        resolve_template("{{ }}", &values, LABEL, 128),
        Err("URL变量名无效".into())
    );
    let long_name = format!("{{{{{}}}}}", "a".repeat(257));
    assert_eq!(
        resolve_template(&long_name, &values, LABEL, 1024),
        Err("URL变量名无效".into())
    );
}

#[test]
fn an_over_budget_literal_fails_before_a_later_missing_variable() {
    let values = variables();

    assert_eq!(
        resolve_template("long{{missing}}", &values, LABEL, 3),
        Err("URL展开后超过 3 bytes 上限".into())
    );
}

#[test]
fn rejected_segment_does_not_change_output_or_allocate_more_capacity() {
    let mut output = String::with_capacity(4);
    output.push_str("ab");
    let capacity = output.capacity();

    assert_eq!(
        append_template_segment(&mut output, "雪", LABEL, 4),
        Err("URL展开后超过 4 bytes 上限".into())
    );
    assert_eq!(output, "ab");
    assert_eq!(output.capacity(), capacity);
}
