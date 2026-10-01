use super::*;
use ramag_infra_system::{Availability, ProcessIdentity, ProcessRow};

fn reading(value: Option<f64>, availability: Availability) -> Reading {
    Reading {
        sensor_id: String::new(),
        value,
        total: None,
        availability,
        reason: None,
        observations: Vec::new(),
    }
}

/// Builds one process with the same initial availability for its four sortable readings.
fn row(
    pid: u32,
    start_time_ticks: u64,
    name: &str,
    user: Option<&str>,
    values: [Option<f64>; 4],
    availability: Availability,
) -> ProcessRow {
    let [cpu, memory, read, write] = values;
    ProcessRow {
        identity: ProcessIdentity {
            pid,
            start_time_ticks,
        },
        name: name.into(),
        user: user.map(str::to_owned),
        user_reason: None,
        cpu_percent: reading(cpu, availability.clone()),
        memory_bytes: reading(memory, availability.clone()),
        read_bytes_per_second: reading(read, availability.clone()),
        write_bytes_per_second: reading(write, availability),
        threads: reading(Some(1.0), Availability::Available),
    }
}

/// Replaces one metric's source state while preserving the other process readings.
fn set_metric_availability(row: &mut ProcessRow, sort: ProcessSort, availability: Availability) {
    let reading = match sort {
        ProcessSort::Cpu => &mut row.cpu_percent,
        ProcessSort::Memory => &mut row.memory_bytes,
        ProcessSort::Read => &mut row.read_bytes_per_second,
        ProcessSort::Write => &mut row.write_bytes_per_second,
        ProcessSort::Pid | ProcessSort::Name | ProcessSort::User => return,
    };
    reading.availability = availability;
}

fn snapshot(processes: Vec<ProcessRow>) -> MonitorSnapshot {
    MonitorSnapshot {
        host: Snapshot {
            processes,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn pids(rows: Vec<&ProcessRow>) -> Vec<u32> {
    rows.into_iter().map(|row| row.identity.pid).collect()
}

#[test]
fn seven_columns_have_stable_ids_labels_and_expected_initial_directions() {
    assert_eq!(ProcessSort::ALL.len(), 7);
    assert_eq!(
        ProcessSort::ALL.map(ProcessSort::stable_id),
        ["cpu", "memory", "pid", "name", "user", "read", "write"]
    );
    assert_eq!(
        ProcessSort::ALL.map(ProcessSort::label),
        ["CPU", "内存", "PID", "名称", "用户", "读取速率", "写入速率"]
    );
    for (sort, direction) in [
        (ProcessSort::Cpu, ProcessSortDirection::Descending),
        (ProcessSort::Memory, ProcessSortDirection::Descending),
        (ProcessSort::Read, ProcessSortDirection::Descending),
        (ProcessSort::Write, ProcessSortDirection::Descending),
        (ProcessSort::Pid, ProcessSortDirection::Ascending),
        (ProcessSort::Name, ProcessSortDirection::Ascending),
        (ProcessSort::User, ProcessSortDirection::Ascending),
    ] {
        assert_eq!(sort.default_direction(), direction);
    }
}

#[test]
fn all_columns_sort_in_both_directions_and_cpu_can_exceed_one_hundred() {
    let data = snapshot(vec![
        row(
            20,
            1,
            "middle",
            Some("b"),
            [Some(80.0), Some(20.0), Some(400.0), Some(4.0)],
            Availability::Available,
        ),
        row(
            30,
            1,
            "high",
            Some("c"),
            [Some(140.0), Some(30.0), Some(900.0), Some(9.0)],
            Availability::Available,
        ),
        row(
            10,
            1,
            "low",
            Some("a"),
            [Some(0.0), Some(10.0), Some(100.0), Some(1.0)],
            Availability::Available,
        ),
    ]);
    for (sort, ascending, descending) in [
        (ProcessSort::Cpu, vec![10, 20, 30], vec![30, 20, 10]),
        (ProcessSort::Memory, vec![10, 20, 30], vec![30, 20, 10]),
        (ProcessSort::Pid, vec![10, 20, 30], vec![30, 20, 10]),
        (ProcessSort::Name, vec![30, 10, 20], vec![20, 10, 30]),
        (ProcessSort::User, vec![10, 20, 30], vec![30, 20, 10]),
        (ProcessSort::Read, vec![10, 20, 30], vec![30, 20, 10]),
        (ProcessSort::Write, vec![10, 20, 30], vec![30, 20, 10]),
    ] {
        assert_eq!(
            pids(data.sorted_processes("", sort, ProcessSortDirection::Ascending)),
            ascending,
            "ascending {sort:?}"
        );
        assert_eq!(
            pids(data.sorted_processes("", sort, ProcessSortDirection::Descending)),
            descending,
            "descending {sort:?}"
        );
    }
}

#[test]
fn cpu_missing_values_follow_valid_5_3_0_and_user_absence_stays_last() {
    let data = snapshot(vec![
        row(40, 1, "missing", None, [None; 4], Availability::Failed),
        row(
            30,
            1,
            "zero",
            Some("c"),
            [Some(0.0); 4],
            Availability::Available,
        ),
        row(
            20,
            1,
            "three",
            Some("b"),
            [Some(3.0); 4],
            Availability::Available,
        ),
        row(
            10,
            1,
            "five",
            Some("a"),
            [Some(5.0); 4],
            Availability::Available,
        ),
    ]);
    assert_eq!(
        pids(data.sorted_processes("", ProcessSort::Cpu, ProcessSortDirection::Descending)),
        vec![10, 20, 30, 40]
    );
    assert_eq!(
        pids(data.sorted_processes("", ProcessSort::User, ProcessSortDirection::Ascending)),
        vec![10, 20, 30, 40]
    );
    assert_eq!(
        pids(data.sorted_processes("", ProcessSort::User, ProcessSortDirection::Descending)),
        vec![30, 20, 10, 40]
    );
}

#[test]
fn every_numeric_column_keeps_invalid_and_missing_readings_last_in_both_directions() {
    let sorts = [
        ProcessSort::Cpu,
        ProcessSort::Memory,
        ProcessSort::Read,
        ProcessSort::Write,
    ];
    for (sort, missing_id) in sorts.into_iter().zip([6_u32, 7, 8, 9]) {
        let mut rows = vec![
            row(1, 1, "valid", None, [Some(5.0); 4], Availability::Available),
            row(2, 1, "failed", None, [Some(99.0); 4], Availability::Failed),
            row(
                3,
                1,
                "warming",
                None,
                [Some(99.0); 4],
                Availability::WarmingUp,
            ),
            row(
                4,
                1,
                "unavailable",
                None,
                [Some(99.0); 4],
                Availability::Unavailable,
            ),
            row(
                5,
                1,
                "nonfinite",
                None,
                [
                    Some(f64::NAN),
                    Some(f64::INFINITY),
                    Some(f64::NEG_INFINITY),
                    Some(f64::NAN),
                ],
                Availability::Available,
            ),
        ];
        for (missing_sort, missing_id) in sorts.into_iter().zip([6_u32, 7, 8, 9]) {
            let mut missing = row(
                missing_id,
                1,
                "missing",
                None,
                [Some(2.0); 4],
                Availability::Available,
            );
            set_metric_availability(&mut missing, missing_sort, Availability::Unavailable);
            let reading = match missing_sort {
                ProcessSort::Cpu => &mut missing.cpu_percent,
                ProcessSort::Memory => &mut missing.memory_bytes,
                ProcessSort::Read => &mut missing.read_bytes_per_second,
                ProcessSort::Write => &mut missing.write_bytes_per_second,
                ProcessSort::Pid | ProcessSort::Name | ProcessSort::User => unreachable!(),
            };
            reading.value = None;
            rows.push(missing);
        }
        let data = snapshot(rows);
        let other_valid = (6..10).filter(|pid| *pid != missing_id).collect::<Vec<_>>();
        for direction in [
            ProcessSortDirection::Ascending,
            ProcessSortDirection::Descending,
        ] {
            let ordered = pids(data.sorted_processes("", sort, direction));
            let expected = match direction {
                ProcessSortDirection::Ascending => {
                    [other_valid.clone(), vec![1], vec![2, 3, 4, 5, missing_id]].concat()
                }
                ProcessSortDirection::Descending => {
                    [vec![1], other_valid.clone(), vec![2, 3, 4, 5, missing_id]].concat()
                }
            };
            assert_eq!(ordered, expected, "{sort:?} {direction:?}");
        }
    }
}

#[test]
fn stale_collection_hides_each_numeric_measurement() {
    let data = snapshot(vec![
        row(1, 1, "high", None, [Some(9.0); 4], Availability::Available),
        row(2, 1, "low", None, [Some(1.0); 4], Availability::Available),
    ]);
    let mut stale = data;
    stale.collection_stale = true;
    for sort in [
        ProcessSort::Cpu,
        ProcessSort::Memory,
        ProcessSort::Read,
        ProcessSort::Write,
    ] {
        for direction in [
            ProcessSortDirection::Ascending,
            ProcessSortDirection::Descending,
        ] {
            assert_eq!(
                pids(stale.sorted_processes("", sort, direction)),
                vec![1, 2],
                "stale readings tie by identity for {sort:?} {direction:?}"
            );
        }
    }
}

#[test]
fn equal_values_use_pid_then_full_process_identity() {
    let data = snapshot(vec![
        row(2, 9, "same", None, [Some(1.0); 4], Availability::Available),
        row(1, 8, "same", None, [Some(1.0); 4], Availability::Available),
        row(1, 3, "same", None, [Some(1.0); 4], Availability::Available),
    ]);
    for sort in ProcessSort::ALL
        .into_iter()
        .filter(|sort| *sort != ProcessSort::Pid)
    {
        for direction in [
            ProcessSortDirection::Ascending,
            ProcessSortDirection::Descending,
        ] {
            let ordered = data.sorted_processes("", sort, direction);
            assert_eq!(
                ordered
                    .iter()
                    .map(|row| (row.identity.pid, row.identity.start_time_ticks))
                    .collect::<Vec<_>>(),
                vec![(1, 3), (1, 8), (2, 9)],
                "identity tie-break for {sort:?} {direction:?}"
            );
        }
    }
}

#[test]
fn search_matches_chinese_name_and_user_and_case_insensitive_latin_and_pid() {
    let data = snapshot(vec![
        row(
            42,
            1,
            "Écho服務",
            Some("管理员"),
            [Some(0.0); 4],
            Availability::Available,
        ),
        row(
            7,
            1,
            "worker",
            Some("Alice"),
            [Some(0.0); 4],
            Availability::Available,
        ),
    ]);
    for (query, expected) in [
        ("服務", vec![42]),
        ("管理员", vec![42]),
        ("42", vec![42]),
        ("ALICE", vec![7]),
    ] {
        assert_eq!(
            pids(data.sorted_processes(query, ProcessSort::Pid, ProcessSortDirection::Ascending)),
            expected
        );
    }
}

#[test]
fn selecting_same_sort_toggles_and_new_sort_uses_column_default() {
    let monitor = SystemMonitor::with_snapshot(MonitorSnapshot::default());
    assert_eq!(
        monitor.process_sort_direction(),
        ProcessSortDirection::Descending
    );
    monitor.set_process_sort(ProcessSort::Pid);
    assert_eq!(
        monitor.process_sort_direction(),
        ProcessSortDirection::Ascending
    );
    monitor.set_process_sort(ProcessSort::Pid);
    assert_eq!(
        monitor.process_sort_direction(),
        ProcessSortDirection::Descending
    );
    monitor.set_process_sort(ProcessSort::Name);
    assert_eq!(
        monitor.process_sort_direction(),
        ProcessSortDirection::Ascending
    );
}
