use super::{MAX_QUERY_WARNINGS, Warning};

/// Append server warnings within the shared limit, reserving a slot for truncation.
pub(super) fn append_warnings_bounded(accumulated: &mut Vec<Warning>, incoming: Vec<Warning>) {
    if incoming.is_empty() {
        return;
    }
    let remaining = MAX_QUERY_WARNINGS.saturating_sub(accumulated.len());
    if incoming.len() <= remaining {
        accumulated.extend(incoming);
        return;
    }

    // 为截断提示预留一格。
    if remaining == 0 {
        accumulated.pop();
    } else {
        accumulated.extend(incoming.into_iter().take(remaining.saturating_sub(1)));
    }
    accumulated.push(Warning {
        level: "Client".into(),
        code: 0,
        message: format!(
            "警告数量超过 {MAX_QUERY_WARNINGS} 条，仅保留前 {} 条以控制资源占用",
            MAX_QUERY_WARNINGS - 1
        ),
    });
}
