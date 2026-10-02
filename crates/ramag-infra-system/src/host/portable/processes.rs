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
        // An inaccessible process cannot prove that its PID changed.
        _ => false,
    }
}

fn process_cache_requires_reset(
    cached: &BTreeMap<u32, u64>,
    current: &BTreeMap<u32, PreviousProcess>,
    pids: impl IntoIterator<Item = u32>,
) -> bool {
    pids.into_iter().any(|pid| {
        let current_ticks = current.get(&pid).map(|process| process.native_ticks);
        cached_pid_requires_reset(cached.get(&pid).copied(), current_ticks)
    })
}

pub(super) struct ProcessIdentitySnapshot {
    previous: BTreeMap<u32, PreviousProcess>,
    pub(super) cache_reset: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProcessIdentityState {
    Stable(u64),
    Unavailable(&'static str),
}

fn stable_process_identity(
    before: Option<PreviousProcess>,
    after: Option<(u64, u64)>,
    sysinfo_start: u64,
    cache_reset: bool,
) -> ProcessIdentityState {
    let Some((native_after, unix_after)) = after else {
        return ProcessIdentityState::Unavailable("native process identity is unavailable");
    };
    let Some(before) = before else {
        return ProcessIdentityState::Unavailable("waiting for a second identity observation");
    };
    if before.native_ticks != native_after
        || before.unix_seconds != unix_after
        || (!cache_reset && before.sysinfo_seconds != sysinfo_start)
        || unix_after != sysinfo_start
    {
        return ProcessIdentityState::Unavailable(
            "process identity changed during the sysinfo refresh",
        );
    }
    ProcessIdentityState::Stable(native_after)
}

fn invalidate_process_counter(counters: &mut Counters, key: &str, reason: &str) {
    let _ = counters.derive(key, Err(reason.to_owned()), |_a, _b, _elapsed| {
        unreachable!("failed observations do not invoke the counter formula")
    });
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
        let cache_reset = process_cache_requires_reset(
            &self.portable_process_identities,
            &previous,
            self.process_system
                .processes()
                .keys()
                .map(|pid| pid.as_u32()),
        );
        if cache_reset {
            self.process_system = sysinfo::System::new();
        }
        ProcessIdentitySnapshot {
            previous,
            cache_reset,
        }
    }

    /// Refreshes process metrics and keeps every sysinfo row. Only matching native identities
    /// receive a nonzero action identity; other rows remain visible and their rates restart.
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
        let mut observed = self.portable_process_identities.clone();
        let mut unverified = 0usize;
        for (pid, process) in self.process_system.processes() {
            let process_id = pid.as_u32();
            let native_after =
                crate::process_control::native_start_time_with_unix_seconds(process_id);
            let (start_time_ticks, identity_reason) = match stable_process_identity(
                identity_snapshot.previous.get(&process_id).copied(),
                native_after,
                process.start_time(),
                identity_snapshot.cache_reset,
            ) {
                ProcessIdentityState::Stable(ticks) => {
                    observed.insert(process_id, ticks);
                    (ticks, None)
                }
                ProcessIdentityState::Unavailable(reason) => {
                    unverified += 1;
                    (0, Some(reason))
                }
            };
            let identity = ProcessIdentity {
                pid: process_id,
                start_time_ticks,
            };
            let key = format!("process:{}:{}", identity.pid, identity.start_time_ticks);
            let source = "sysinfo::Process::accumulated_cpu_time (milliseconds)";
            let cpu_key = format!("{key}/cpu");
            let cpu_percent = if let Some(reason) = identity_reason {
                invalidate_process_counter(&mut self.counters, &cpu_key, reason);
                missing(&cpu_key, Availability::WarmingUp, reason.into())
            } else {
                self.counters.derive(
                    &cpu_key,
                    Ok(api_raw(
                        source,
                        window,
                        [("cpu_ms", process.accumulated_cpu_time())],
                    )),
                    |a, b, elapsed| Ok(delta(a, b, "cpu_ms")? as f64 / 10.0 / elapsed),
                )
            };
            let disk = process.disk_usage();
            let mut rate = |suffix: &str, bytes: u64| {
                let counter_key = format!("{key}/{suffix}");
                if let Some(reason) = identity_reason {
                    invalidate_process_counter(&mut self.counters, &counter_key, reason);
                    return missing(&counter_key, Availability::WarmingUp, reason.into());
                }
                self.counters.derive(
                    &counter_key,
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
        let live_pids: std::collections::BTreeSet<_> = self
            .process_system
            .processes()
            .keys()
            .map(|pid| pid.as_u32())
            .collect();
        observed.retain(|pid, _| live_pids.contains(pid));
        self.portable_process_identities = observed;
        if unverified > 0 {
            snapshot.diagnostics.push(BackendDiagnostic {
                backend: "sysinfo-process-identities".into(),
                availability: Availability::WarmingUp,
                reason: format!(
                    "{unverified} enumerated process row(s) have identity 0 and are read-only; process counter rates restart when identity is unavailable"
                ),
            });
        }
        let processes = api_integer(
            "cpu:host/processes",
            "sysinfo::System::processes",
            candidate_count as u64,
            window,
        );
        sensor(
            snapshot,
            ("cpu:host", "processes", "Processes"),
            SensorKind::Counter,
            Unit::Count,
            "sysinfo::System::processes",
            "All enumerated sysinfo processes; rows with identity 0 are read-only",
            processes,
        );
        let threads = thread_total.map_or_else(
            || {
                missing(
                    "cpu:host/threads",
                    Availability::Unavailable,
                    "Thread counts are not exposed for all processes by sysinfo".into(),
                )
            },
            |count| api_integer("cpu:host/threads", "sysinfo::Process::tasks", count, window),
        );
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
    use super::{
        PreviousProcess, ProcessIdentityState, cached_pid_requires_reset,
        process_cache_requires_reset, stable_process_identity,
    };
    use std::collections::BTreeMap;

    #[test]
    fn reused_pid_or_changed_cached_sysinfo_record_is_rejected() {
        let original = PreviousProcess {
            native_ticks: 700,
            unix_seconds: 42,
            sysinfo_seconds: 42,
        };
        assert_eq!(
            stable_process_identity(Some(original), Some((700, 42)), 42, false),
            ProcessIdentityState::Stable(700)
        );
        assert_eq!(
            stable_process_identity(Some(original), Some((701, 42)), 42, false),
            ProcessIdentityState::Unavailable(
                "process identity changed during the sysinfo refresh"
            )
        );
        assert_eq!(
            stable_process_identity(Some(original), Some((700, 42)), 43, false),
            ProcessIdentityState::Unavailable(
                "process identity changed during the sysinfo refresh"
            )
        );
        assert_eq!(
            stable_process_identity(Some(original), Some((700, 43)), 42, true),
            ProcessIdentityState::Unavailable(
                "process identity changed during the sysinfo refresh"
            )
        );
        assert_eq!(
            stable_process_identity(None, Some((700, 42)), 42, false),
            ProcessIdentityState::Unavailable("waiting for a second identity observation")
        );
        assert_eq!(
            stable_process_identity(Some(original), None, 42, false),
            ProcessIdentityState::Unavailable("native process identity is unavailable")
        );
    }

    #[test]
    fn same_second_native_reuse_forces_fresh_sysinfo_cache_before_acceptance() {
        let old_ticks = 133_444_736_000_000_001;
        let new_ticks = old_ticks + 1;
        assert_eq!(old_ticks / 10_000_000, new_ticks / 10_000_000);
        assert!(!cached_pid_requires_reset(Some(old_ticks), Some(old_ticks)));
        assert!(cached_pid_requires_reset(Some(old_ticks), Some(new_ticks)));
        assert!(!cached_pid_requires_reset(Some(old_ticks), None));
        assert!(!cached_pid_requires_reset(None, Some(new_ticks)));

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
            ProcessIdentityState::Stable(new_ticks)
        );
    }

    #[test]
    fn inaccessible_identity_does_not_reset_the_process_cache() {
        assert_eq!(
            stable_process_identity(None, None, 0, false),
            ProcessIdentityState::Unavailable("native process identity is unavailable")
        );
        assert_eq!(
            stable_process_identity(None, Some((700, 42)), 42, false),
            ProcessIdentityState::Unavailable("waiting for a second identity observation")
        );
        assert!(!cached_pid_requires_reset(None, None));
    }

    #[test]
    fn permanently_inaccessible_pid_does_not_repeatedly_reset_process_cache() {
        let cached = BTreeMap::from([(77, 700)]);
        for _ in 0..4 {
            assert!(!process_cache_requires_reset(
                &cached,
                &BTreeMap::new(),
                [77]
            ));
        }
    }

    #[test]
    fn zero_identity_is_refused_by_process_control() {
        let result = crate::process_control::send_signal(
            &crate::ProcessIdentity {
                pid: 77,
                start_time_ticks: 0,
            },
            crate::process_control::ProcessSignal::Kill,
        );
        assert_eq!(
            result,
            Err("The selected process has no verified start identity".into())
        );
    }
}
