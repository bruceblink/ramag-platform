use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// Returns resident and virtual memory in bytes for this process, rejecting an unusable sample.
pub fn current_process_memory() -> Result<(u64, u64), String> {
    let pid = Pid::from_u32(std::process::id());
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_memory(),
    );
    let process = system
        .process(pid)
        .ok_or_else(|| "Current process memory is unavailable".to_string())?;
    let sample = (process.memory(), process.virtual_memory());
    if sample.0 == 0 && sample.1 == 0 {
        return Err("Current process memory returned no usable measurements".into());
    }
    Ok(sample)
}

#[cfg(test)]
mod tests {
    use super::current_process_memory;

    #[test]
    fn current_process_has_at_least_one_usable_memory_measurement() -> Result<(), std::io::Error> {
        let (resident, virtual_bytes) = current_process_memory().map_err(std::io::Error::other)?;
        assert!(resident > 0 || virtual_bytes > 0);
        Ok(())
    }
}
