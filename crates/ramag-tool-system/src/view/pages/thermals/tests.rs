use std::collections::{BTreeMap, VecDeque};

use ramag_infra_system::{SensorDescriptor, SensorKind, Snapshot, Unit};

use crate::{MonitorSnapshot, ReadingStatus, SensorSample};

use super::{hottest_current_temperature, selected_thermal_sensor};

// Builds a descriptor with stable identity and an explicitly declared sensor category.
fn sensor(id: &str, kind: SensorKind, unit: Unit) -> SensorDescriptor {
    SensorDescriptor {
        id: id.into(),
        monitor_id: "thermal-tests".into(),
        title: id.into(),
        kind,
        unit,
        source: "test-source".into(),
        scope: "test-scope".into(),
        scale: None,
    }
}

// Creates one-sample histories so tests can vary both value validity and collection status.
fn snapshot(samples: &[(&str, ReadingStatus, Option<f64>)]) -> MonitorSnapshot {
    MonitorSnapshot {
        host: Snapshot::default(),
        histories: samples
            .iter()
            .map(|(id, status, value)| {
                (
                    (*id).into(),
                    VecDeque::from([SensorSample {
                        at_seconds: 1.0,
                        value: *value,
                        total: None,
                        status: *status,
                        reason: None,
                    }]),
                )
            })
            .collect::<BTreeMap<_, _>>(),
        ..MonitorSnapshot::default()
    }
}

#[test]
fn hottest_current_temperature_uses_finite_current_celsius_temperature_and_stable_ties() {
    let cpu = sensor("cpu", SensorKind::Temperature, Unit::Celsius);
    let gpu = sensor("gpu", SensorKind::Temperature, Unit::Celsius);
    let readings = [&gpu, &cpu];
    let current = snapshot(&[
        ("cpu", ReadingStatus::Current, Some(82.0)),
        ("gpu", ReadingStatus::Current, Some(82.0)),
    ]);

    assert_eq!(
        hottest_current_temperature(&readings, &current)
            .map(|(descriptor, value)| (descriptor.id.as_str(), value)),
        Some(("cpu", 82.0)),
        "equal temperatures must choose the lexicographically smaller stable ID"
    );

    let unusual_but_valid = snapshot(&[
        ("cpu", ReadingStatus::Current, Some(0.0)),
        ("gpu", ReadingStatus::Current, Some(-4.5)),
    ]);
    assert_eq!(
        hottest_current_temperature(&readings, &unusual_but_valid)
            .map(|(descriptor, value)| (descriptor.id.as_str(), value)),
        Some(("cpu", 0.0)),
        "zero and negative Celsius values remain valid current measurements"
    );
}

#[test]
fn hottest_current_temperature_rejects_noncurrent_nonfinite_and_stale_samples() {
    let ids = [
        "unavailable",
        "warming",
        "failed",
        "stale",
        "nan",
        "infinity",
    ];
    let readings = ids
        .iter()
        .map(|id| sensor(id, SensorKind::Temperature, Unit::Celsius))
        .collect::<Vec<_>>();
    let reading_refs = readings.iter().collect::<Vec<_>>();
    let samples = snapshot(&[
        ("unavailable", ReadingStatus::Unavailable, Some(120.0)),
        ("warming", ReadingStatus::WarmingUp, Some(121.0)),
        ("failed", ReadingStatus::Failed, Some(122.0)),
        ("stale", ReadingStatus::Stale, Some(123.0)),
        ("nan", ReadingStatus::Current, Some(f64::NAN)),
        ("infinity", ReadingStatus::Current, Some(f64::INFINITY)),
    ]);
    assert!(
        hottest_current_temperature(&reading_refs, &samples).is_none(),
        "only finite Current readings may be selected as the hottest value"
    );

    let mut stalled_collection = snapshot(&[("unavailable", ReadingStatus::Current, Some(99.0))]);
    stalled_collection.collection_stale = true;
    assert!(
        hottest_current_temperature(&[&readings[0]], &stalled_collection).is_none(),
        "an old sample cannot become current while its collection is stalled"
    );
}

#[test]
fn hottest_current_temperature_ignores_other_kinds_and_units() {
    let temperature = sensor("valid", SensorKind::Temperature, Unit::Celsius);
    let percentage = sensor("percentage", SensorKind::Temperature, Unit::Percent);
    let fan = sensor("fan", SensorKind::Fan, Unit::Celsius);
    let readings = [&percentage, &fan, &temperature];
    let samples = snapshot(&[
        ("valid", ReadingStatus::Current, Some(70.0)),
        ("percentage", ReadingStatus::Current, Some(100.0)),
        ("fan", ReadingStatus::Current, Some(9000.0)),
    ]);

    assert_eq!(
        hottest_current_temperature(&readings, &samples)
            .map(|(descriptor, value)| (descriptor.id.as_str(), value)),
        Some(("valid", 70.0)),
        "the hottest-value helper must only consider Celsius temperature descriptors"
    );
}

#[test]
fn selected_thermal_sensor_preserves_saved_identity_and_falls_back_by_stable_id() {
    let second = sensor("sensor-b", SensorKind::Temperature, Unit::Celsius);
    let first = sensor("sensor-a", SensorKind::Temperature, Unit::Celsius);
    let readings = [&second, &first];
    let samples = snapshot(&[
        ("sensor-a", ReadingStatus::Unavailable, None),
        ("sensor-b", ReadingStatus::Failed, None),
    ]);

    assert_eq!(
        selected_thermal_sensor(&readings, Some("sensor-b"), &samples)
            .map(|descriptor| descriptor.id.as_str()),
        Some("sensor-b"),
        "a saved sensor keeps its identity even when it has no Current value"
    );
    assert!(
        selected_thermal_sensor(&readings, Some("removed"), &samples).is_none(),
        "a missing saved identity must stay unavailable instead of silently changing sensors"
    );
    assert_eq!(
        selected_thermal_sensor(&readings, None, &samples).map(|descriptor| descriptor.id.as_str()),
        Some("sensor-a"),
        "when no sensor has a Current sample, fallback uses the smallest stable ID"
    );
}
