use super::*;
use std::collections::BTreeMap;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, UpdateKind};

#[derive(Clone, Copy)]
struct PreviousProcess {
    native_ticks: u64,
    unix_seconds: u64,
    sysinfo_seconds: u64,
}

fn cached_pid_requires_reset(known_ticks: Option<u64>, current_ticks: Option<u64>) -> bool {
    match (known_ticks, current_ticks) {
        (Some(known), Some(current)) => known != current,
        _ => true,
    }
}

pub(super) struct ProcessIdentitySnapshot {
    previous: BTreeMap<u32, PreviousProcess>,
    pub(super) cache_reset: bool,
}

fn stable_process_identity(
    before: Option<PreviousProcess>,
    after: Option<(u64, u64)>,
    sysinfo_start: u64,
    cache_reset: bool,
) -> Option<u64> {
    let before = before?;
    let (native_after, unix_after) = after?;
    if before.native_ticks != native_after
        || before.unix_seconds != unix_after
        || (!cache_reset && before.sysinfo_seconds != sysinfo_start)
        || unix_after != sysinfo_start
    {
        return None;
    }
    Some(native_after)
}

impl HostCollector {
    /// Captures identities from the cached PID set before refresh. If a PID was reused, reset
    /// only the process System so stale process records are discarded without warming host CPU
    /// counters again.
    pub(super) fn prepare_portable_process_refresh(&mut self) -> ProcessIdentitySnapshot {
        let previous: BTreeMap<_, _> = self
            .process_system
            .processes()
            .iter()
            .filter_map(|(pid, process)| {
                crate::process_control::native_start_time_with_unix_seconds(pid.as_u32()).map(
                    |(native_ticks, unix_seconds)| {
                        (
                            pid.as_u32(),
                            PreviousProcess {
                                native_ticks,
                                unix_seconds,
                                sysinfo_seconds: process.start_time(),
                            },
                        )
                    },
                )
            })
            .collect();
        let cache_reset = self.process_system.processes().keys().any(|pid| {
            let current = previous
                .get(&pid.as_u32())
                .map(|process| process.native_ticks);
            cached_pid_requires_reset(
                self.portable_process_identities.get(&pid.as_u32()).copied(),
                current,
            )
        });
        if cache_reset {
            self.process_system = sysinfo::System::new();
        }
        ProcessIdentitySnapshot {
            previous,
            cache_reset,
        }
    }

    /// Refreshes process metrics and publishes only rows whose exact native identity and
    /// sysinfo Unix-second start value agree with the pre-refresh process. Unstable PIDs are
    /// omitted; their metric baselines are pruned by the collector's end-of-capture cleanup.
    pub(super) fn collect_portable_processes(
        &mut self,
        snapshot: &mut Snapshot,
        identity_snapshot: ProcessIdentitySnapshot,
        clock: impl Fn() -> u64,
    ) -> QueryWindow {
        let (_, window) = query(clock, || {
            self.process_system.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::nothing()
                    .with_cpu()
                    .with_memory()
                    .with_disk_usage()
                    .with_user(UpdateKind::Always)
                    .with_tasks(),
            )
        });
        let users = sysinfo::Users::new_with_refreshed_list();
        let mut thread_total = Some(0u64);
        let candidate_count = self.process_system.processes().len();
        let mut observed = BTreeMap::new();
        for (pid, process) in self.process_system.processes() {
            let process_id = pid.as_u32();
            let native_after =
                crate::process_control::native_start_time_with_unix_seconds(process_id);
            let Some(start_time_ticks) = stable_process_identity(
                identity_snapshot.previous.get(&process_id).copied(),
                native_after,
                process.start_time(),
                identity_snapshot.cache_reset,
            ) else {
                continue;
            };
            observed.insert(process_id, start_time_ticks);
            let identity = ProcessIdentity {
                pid: process_id,
                start_time_ticks,
            };
            let key = format!("process:{}:{}", identity.pid, identity.start_time_ticks);
            let source = "sysinfo::Process::accumulated_cpu_time (milliseconds)";
            let cpu_percent = self.counters.derive(
                &format!("{key}/cpu"),
                Ok(api_raw(
                    source,
                    window,
                    [("cpu_ms", process.accumulated_cpu_time())],
                )),
                |a, b, elapsed| Ok(delta(a, b, "cpu_ms")? as f64 / 10.0 / elapsed),
            );
            let disk = process.disk_usage();
            let mut rate = |suffix: &str, bytes: u64| {
                self.counters.derive(
                    &format!("{key}/{suffix}"),
                    Ok(api_raw(
                        "sysinfo::Process::disk_usage",
                        window,
                        [("bytes", bytes)],
                    )),
                    |a, b, elapsed| Ok(delta(a, b, "bytes")? as f64 / elapsed),
                )
            };
            let read_bytes_per_second = rate("read", disk.total_read_bytes);
            let write_bytes_per_second = rate("write", disk.total_written_bytes);
            let user = process
                .user_id()
                .and_then(|id| users.get_user_by_id(id))
                .map(|user| user.name().to_string());
            let count = process.tasks().map(|tasks| tasks.len() as u64);
            thread_total = thread_total
                .zip(count)
                .and_then(|(total, count)| total.checked_add(count));
            let threads = count.map_or_else(
                || {
                    missing(
                        &format!("{key}/threads"),
                        Availability::Unavailable,
                        "sysinfo does not expose this process's thread set".into(),
                    )
                },
                |count| {
                    api_integer(
                        &format!("{key}/threads"),
                        "sysinfo::Process::tasks",
                        count,
                        window,
                    )
                },
            );
            snapshot.processes.push(ProcessRow {
                identity,
                name: process.name().to_string_lossy().into_owned(),
                user_reason: user
                    .is_none()
                    .then(|| "sysinfo process user unavailable".into()),
                user,
                cpu_percent,
                memory_bytes: api_integer(
                    &format!("{key}/memory"),
                    "sysinfo::Process::memory",
                    process.memory(),
                    window,
                ),
                read_bytes_per_second,
                write_bytes_per_second,
                threads,
            });
        }
        self.portable_process_identities = observed;
        let identity_pending = snapshot.processes.len() < candidate_count;
        let processes = if identity_pending {
            missing(
                "cpu:host/processes",
                Availability::WarmingUp,
                "Waiting for stable native identities before publishing the process count".into(),
            )
        } else {
            api_integer(
                "cpu:host/processes",
                "sysinfo::System::processes",
                snapshot.processes.len() as u64,
                window,
            )
        };
        sensor(
            snapshot,
            ("cpu:host", "processes", "Processes"),
            SensorKind::Counter,
            Unit::Count,
            "sysinfo::System::processes",
            if identity_pending {
                "Count waits for stable native identities"
            } else {
                "Enumerated processes with stable native identities"
            },
            processes,
        );
        let threads = if identity_pending {
            missing(
                "cpu:host/threads",
                Availability::WarmingUp,
                "Waiting for stable native identities before publishing thread totals".into(),
            )
        } else {
            thread_total.map_or_else(
                || {
                    missing(
                        "cpu:host/threads",
                        Availability::Unavailable,
                        "Thread counts are not exposed for all processes by sysinfo".into(),
                    )
                },
                |count| api_integer("cpu:host/threads", "sysinfo::Process::tasks", count, window),
            )
        };
        sensor(
            snapshot,
            ("cpu:host", "threads", "Threads"),
            SensorKind::Counter,
            Unit::Count,
            "sysinfo::Process::tasks",
            "Sum of enumerated process threads",
            threads,
        );
        window
    }
}

#[cfg(test)]
mod tests {
    use super::{PreviousProcess, cached_pid_requires_reset, stable_process_identity};

    #[test]
    fn reused_pid_or_changed_cached_sysinfo_record_is_rejected() {
        let original = PreviousProcess {
            native_ticks: 700,
            unix_seconds: 42,
            sysinfo_seconds: 42,
        };
        assert_eq!(
            stable_process_identity(Some(original), Some((700, 42)), 42, false),
            Some(700)
        );
        assert_eq!(
            stable_process_identity(Some(original), Some((701, 42)), 42, false),
            None
        );
        assert_eq!(
            stable_process_identity(Some(original), Some((700, 42)), 43, false),
            None
        );
        assert_eq!(
            stable_process_identity(Some(original), Some((700, 43)), 42, true),
            None
        );
        assert_eq!(
            stable_process_identity(None, Some((700, 42)), 42, false),
            None
        );
        assert_eq!(
            stable_process_identity(Some(original), None, 42, false),
            None
        );
    }

    #[test]
    fn same_second_native_reuse_forces_fresh_sysinfo_cache_before_acceptance() {
        let old_ticks = 133_444_736_000_000_001;
        let new_ticks = old_ticks + 1;
        assert_eq!(old_ticks / 10_000_000, new_ticks / 10_000_000);
        assert!(!cached_pid_requires_reset(Some(old_ticks), Some(old_ticks)));
        assert!(cached_pid_requires_reset(Some(old_ticks), Some(new_ticks)));
        assert!(cached_pid_requires_reset(Some(old_ticks), None));
        assert!(cached_pid_requires_reset(None, Some(new_ticks)));

        let before_refresh = PreviousProcess {
            native_ticks: new_ticks,
            unix_seconds: 1_700_000_000,
            sysinfo_seconds: 1_699_999_999,
        };
        assert_eq!(
            stable_process_identity(
                Some(before_refresh),
                Some((new_ticks, 1_700_000_000)),
                1_700_000_000,
                true,
            ),
            Some(new_ticks)
        );
    }
}
