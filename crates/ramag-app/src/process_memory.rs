//! 当前进程的轻量内存采样；不读取其它进程，也不改变任务预算。

use sysinfo::{Pid, ProcessesToUpdate, System};

/// 一次当前进程内存采样。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessMemorySnapshot {
    /// 被当前进程占用的常驻内存字节数。
    pub resident_bytes: u64,
    /// 当前进程向操作系统申请的虚拟内存字节数。
    pub virtual_bytes: u64,
    /// 采样对应的操作系统进程 ID。
    pub pid: u32,
}

/// 读取当前进程的常驻内存和虚拟内存。
///
/// 某些系统或受限运行环境可能无法返回当前进程记录，此时返回 `None`；调用方不应把
/// 缺失样本解释为零，也不应因此改变插件任务的生命周期和资源上限。
pub fn current_process_memory() -> Option<ProcessMemorySnapshot> {
    let pid = Pid::from_u32(std::process::id());
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    let process = system.process(pid)?;
    Some(ProcessMemorySnapshot {
        resident_bytes: process.memory(),
        virtual_bytes: process.virtual_memory(),
        pid: pid.as_u32(),
    })
}

#[cfg(test)]
mod tests {
    use super::current_process_memory;

    #[test]
    fn sample_targets_current_process_when_available() {
        let Some(sample) = current_process_memory() else {
            return;
        };
        assert_eq!(sample.pid, std::process::id());
        // Operating systems report resident and virtual memory with different
        // accounting rules; sysinfo does not guarantee that virtual memory is
        // larger than resident memory on every platform. Require at least one
        // usable metric while keeping the test focused on the current process.
        assert!(sample.resident_bytes > 0 || sample.virtual_bytes > 0);
    }
}
