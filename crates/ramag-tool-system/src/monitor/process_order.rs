use std::cmp::Ordering;

use ramag_infra_system::ProcessRow;

use super::{MonitorSnapshot, ProcessSort, ProcessSortDirection};

impl ProcessSort {
    /// New text and identity columns start ascending; measured resource columns start descending.
    pub fn default_direction(self) -> ProcessSortDirection {
        match self {
            Self::Cpu | Self::Memory | Self::Read | Self::Write => ProcessSortDirection::Descending,
            Self::Pid | Self::Name | Self::User => ProcessSortDirection::Ascending,
        }
    }
}

impl MonitorSnapshot {
    /// Filters by name, PID, or user, then sorts rows with missing readings always last.
    /// Numeric metrics use validated physical values; identity fields break ties independently
    /// of the collector's row order so refreshes do not shuffle equal-valued processes.
    pub fn sorted_processes(
        &self,
        query: &str,
        sort: ProcessSort,
        direction: ProcessSortDirection,
    ) -> Vec<&ProcessRow> {
        let query = query.to_lowercase();
        let mut rows = self
            .host
            .processes
            .iter()
            .filter(|row| {
                query.is_empty()
                    || row.name.to_lowercase().contains(&query)
                    || row.identity.pid.to_string().contains(&query)
                    || row
                        .user
                        .as_deref()
                        .is_some_and(|user| user.to_lowercase().contains(&query))
            })
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| {
            compare_rows(self, left, right, sort, direction)
                .then_with(|| left.identity.pid.cmp(&right.identity.pid))
                .then_with(|| {
                    left.identity
                        .start_time_ticks
                        .cmp(&right.identity.start_time_ticks)
                })
        });
        rows
    }
}

fn compare_rows(
    snapshot: &MonitorSnapshot,
    left: &ProcessRow,
    right: &ProcessRow,
    sort: ProcessSort,
    direction: ProcessSortDirection,
) -> Ordering {
    // Compare only the selected field here; stable identity tie-breaks belong to the caller.
    match sort {
        ProcessSort::Pid => {
            order_by_direction(left.identity.pid.cmp(&right.identity.pid), direction)
        }
        ProcessSort::Name => compare_text(Some(&left.name), Some(&right.name), direction),
        ProcessSort::User => compare_text(left.user.as_deref(), right.user.as_deref(), direction),
        ProcessSort::Cpu => compare_numeric(
            snapshot.process_value(&left.cpu_percent),
            snapshot.process_value(&right.cpu_percent),
            direction,
        ),
        ProcessSort::Memory => compare_numeric(
            snapshot.process_value(&left.memory_bytes),
            snapshot.process_value(&right.memory_bytes),
            direction,
        ),
        ProcessSort::Read => compare_numeric(
            snapshot.process_value(&left.read_bytes_per_second),
            snapshot.process_value(&right.read_bytes_per_second),
            direction,
        ),
        ProcessSort::Write => compare_numeric(
            snapshot.process_value(&left.write_bytes_per_second),
            snapshot.process_value(&right.write_bytes_per_second),
            direction,
        ),
    }
}

fn compare_text(
    left: Option<&str>,
    right: Option<&str>,
    direction: ProcessSortDirection,
) -> Ordering {
    // Compare case-insensitively while keeping an absent user after every known user.
    match (left, right) {
        (Some(left), Some(right)) => {
            order_by_direction(left.to_lowercase().cmp(&right.to_lowercase()), direction)
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_numeric(
    left: Option<f64>,
    right: Option<f64>,
    direction: ProcessSortDirection,
) -> Ordering {
    // `total_cmp` is safe for every finite input, and `process_value` filters invalid readings.
    match (left, right) {
        (Some(left), Some(right)) => order_by_direction(left.total_cmp(&right), direction),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn order_by_direction(order: Ordering, direction: ProcessSortDirection) -> Ordering {
    // Reverse only valid-value comparisons so absence remains last in both directions.
    match direction {
        ProcessSortDirection::Ascending => order,
        ProcessSortDirection::Descending => order.reverse(),
    }
}
