use super::*;
#[derive(Clone, Copy, Debug)]
pub(super) struct QueryWindow {
    pub(super) started_ns: u64,
    pub(super) captured_ns: u64,
}
pub(super) fn query<T>(clock: impl Fn() -> u64, operation: impl FnOnce() -> T) -> (T, QueryWindow) {
    let started_ns = clock();
    let value = operation();
    (
        value,
        QueryWindow {
            started_ns,
            captured_ns: clock(),
        },
    )
}
pub(super) fn api_raw<const N: usize>(
    source: &str,
    window: QueryWindow,
    values: [(&str, u64); N],
) -> RawObservation {
    raw_window(source, window.started_ns, window.captured_ns, values)
}

#[cfg(any(test, target_os = "macos"))]
pub(super) fn filesystem_capacity_reading(
    id: &str,
    blocks: Result<(u64, u64, u64), String>,
    window: QueryWindow,
) -> Reading {
    let (blocks, free_blocks, fragment_size) = match blocks {
        Ok(values) => values,
        Err(error) => return missing(id, Availability::Failed, error),
    };
    let total = blocks.checked_mul(fragment_size);
    let used = blocks
        .checked_sub(free_blocks)
        .and_then(|used| used.checked_mul(fragment_size));
    let (Some(total), Some(used)) = (total, used) else {
        return missing(
            id,
            Availability::Failed,
            "statvfs capacity arithmetic overflow".into(),
        );
    };
    if fragment_size == 0 || total == 0 {
        return missing(
            id,
            Availability::Unavailable,
            "statvfs reports no filesystem capacity".into(),
        );
    }
    let mut reading = measured(id, used as f64, Some(total as f64));
    reading.observations.push(api_raw(
        "statvfs filesystem capacity",
        window,
        [
            ("blocks", blocks),
            ("free_blocks", free_blocks),
            ("fragment_size", fragment_size),
        ],
    ));
    reading
}
pub(super) fn api_scalar(id: &str, source: &str, value: f64, window: QueryWindow) -> Reading {
    if !value.is_finite() {
        return missing(
            id,
            Availability::Failed,
            format!("{source}: nonfinite API result"),
        );
    }
    let mut r = measured(id, value, None);
    let mut o = api_raw(source, window, []);
    o.decimals.insert("value".into(), value);
    r.observations.push(o);
    r
}
pub(super) fn api_integer(id: &str, source: &str, value: u64, window: QueryWindow) -> Reading {
    let mut r = measured(id, value as f64, None);
    r.observations
        .push(api_raw(source, window, [("value", value)]));
    r
}
/// Publishes a known sensor without a numeric value and preserves the missing-data reason.
pub(super) fn unsupported(
    s: &mut Snapshot,
    identity: (&str, &str, &str),
    kind: SensorKind,
    unit: Unit,
    source: &str,
    reason: &str,
) {
    let (id, suffix, _) = identity;
    sensor(
        s,
        identity,
        kind,
        unit,
        source,
        reason,
        missing(
            &format!("{id}/{suffix}"),
            Availability::Unavailable,
            reason.into(),
        ),
    );
}
