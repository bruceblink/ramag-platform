use ramag_infra_system::ProcessRow;

use crate::{MonitorSnapshot, StableProcessIdentity};

/// Owns one selected lifetime and its original display name, never an index or stale metrics.
/// Sorting, filtering and page changes retain the identity. Each render resolves live values from
/// the whole cached snapshot. Missing rows and PID reuse retain the original target for an explicit
/// explanation until the user closes details or selects another verified lifetime. No OS handles,
/// command arguments, environment or credentials are retained, and no process signal is sent.
#[derive(Clone, Debug)]
pub(super) struct SelectedProcess {
    pub(super) identity: StableProcessIdentity,
    pub(super) name: String,
}

/// Borrows metrics only from the exact lifetime; absence and reuse have no previous or new readings.
pub(super) enum SelectedProcessState<'a> {
    Current(&'a ProcessRow),
    Missing,
    Reused,
}

impl SelectedProcess {
    /// Resolves against unfiltered cached data so search and row limits cannot reassign selection.
    pub(super) fn resolve<'a>(&self, snapshot: &'a MonitorSnapshot) -> SelectedProcessState<'a> {
        if let Some(row) = snapshot
            .host
            .processes
            .iter()
            .find(|row| row.identity == self.identity)
        {
            return SelectedProcessState::Current(row);
        }
        if snapshot
            .host
            .processes
            .iter()
            .any(|row| row.identity.pid == self.identity.pid)
        {
            SelectedProcessState::Reused
        } else {
            SelectedProcessState::Missing
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_infra_system::{Availability, Reading, Snapshot};

    fn snapshot(pid: u32, start_time_ticks: u64) -> MonitorSnapshot {
        let reading = Reading {
            sensor_id: String::new(),
            value: Some(0.0),
            total: None,
            availability: Availability::Available,
            reason: None,
            observations: Vec::new(),
        };
        MonitorSnapshot {
            host: Snapshot {
                processes: vec![ProcessRow {
                    identity: StableProcessIdentity {
                        pid,
                        start_time_ticks,
                    },
                    name: "worker".into(),
                    user: None,
                    user_reason: None,
                    cpu_percent: reading.clone(),
                    memory_bytes: reading.clone(),
                    read_bytes_per_second: reading.clone(),
                    write_bytes_per_second: reading.clone(),
                    threads: reading,
                }],
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Resolves the selected lifetime using full ticks, including values beyond f64 precision.
    #[test]
    fn exact_identity_resolves_live_row_but_missing_and_reuse_never_expose_metrics() {
        let selected = SelectedProcess {
            identity: StableProcessIdentity {
                pid: 4242,
                start_time_ticks: (1_u64 << 54) + 7,
            },
            name: "original".into(),
        };
        let mut data = snapshot(4242, selected.identity.start_time_ticks);
        let live = selected.resolve(&data);
        assert!(matches!(live, SelectedProcessState::Current(_)));
        if let SelectedProcessState::Current(row) = live {
            assert_eq!(row.name, "worker");
            assert_eq!(row.identity, selected.identity);
        }
        data.host.processes[0].identity.start_time_ticks += 1;
        assert!(matches!(
            selected.resolve(&data),
            SelectedProcessState::Reused
        ));
        data.host.processes.clear();
        assert!(matches!(
            selected.resolve(&data),
            SelectedProcessState::Missing
        ));
        data = snapshot(4243, selected.identity.start_time_ticks);
        assert!(matches!(
            selected.resolve(&data),
            SelectedProcessState::Missing
        ));
    }
}
