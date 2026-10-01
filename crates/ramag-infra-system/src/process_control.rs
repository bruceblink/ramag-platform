//! Explicit process actions, separate from the read-only sampling worker.
use crate::ProcessIdentity;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// A requested operating-system signal; callers must confirm destructive actions first.
pub enum ProcessSignal {
    Terminate,
    Kill,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// The outcome reported by native process-control backends.
pub enum ProcessActionOutcome {
    SignalSent,
    ExitObserved,
    CloseRequested,
    TerminationPending,
}

mod authentication;
pub use authentication::{helper_entry, send_signal_with_authentication};

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(any(target_os = "windows", test))]
mod windows_protocol;

#[derive(Debug, PartialEq, Eq)]
enum ActionError {
    PermissionDenied,
    Failed(String),
}

impl From<String> for ActionError {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}

impl From<&str> for ActionError {
    fn from(message: &str) -> Self {
        Self::Failed(message.into())
    }
}

impl std::fmt::Display for ActionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PermissionDenied => {
                formatter.write_str("Permission denied; the process was not changed")
            }
            Self::Failed(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ActionError {}

#[cfg(unix)]
fn os_error(operation: &str, error: std::io::Error) -> ActionError {
    match error.raw_os_error() {
        Some(libc::EPERM | libc::EACCES) => ActionError::PermissionDenied,
        Some(libc::ESRCH | libc::ENOENT) => "The selected process has already exited".into(),
        Some(libc::ENOSYS) => "This kernel does not support identity-safe process actions".into(),
        _ => format!("{operation}: {error}").into(),
    }
}

#[cfg(unix)]
impl From<std::io::Error> for ActionError {
    fn from(error: std::io::Error) -> Self {
        os_error("Process-control test operation", error)
    }
}

fn validate_identity(identity: &ProcessIdentity) -> Result<(), ActionError> {
    if identity.pid <= 1 || identity.pid == std::process::id() || identity.pid > i32::MAX as u32 {
        return Err("This process is protected from task actions".into());
    }
    if identity.start_time_ticks == 0 {
        return Err("The selected process has no verified start identity".into());
    }
    Ok(())
}

pub fn send_signal(identity: &ProcessIdentity, signal: ProcessSignal) -> Result<(), String> {
    send(identity, None, signal).map_err(|error| error.to_string())
}

/// Revalidates the live process name and creation identity before sending a signal.
pub fn send_signal_checked(
    identity: &ProcessIdentity,
    expected_name: &str,
    signal: ProcessSignal,
) -> Result<(), String> {
    if expected_name.is_empty() || expected_name.contains('/') || expected_name.contains('\\') {
        return Err("The selected process name is invalid".into());
    }
    send(identity, Some(expected_name), signal).map_err(|error| error.to_string())
}

/// Returns an exact identity plus its sysinfo-compatible Unix-second value.
#[cfg(any(target_os = "macos", target_os = "windows", test))]
pub(crate) fn native_start_time_with_unix_seconds(pid: u32) -> Option<(u64, u64)> {
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let ticks = linux::parse_identity(&text).ok()?.start_time_ticks;
        let boot_time = linux::boot_time_unix_seconds()?;
        let hz = rustix::param::clock_ticks_per_second();
        Some((ticks, boot_time.checked_add(ticks / hz)?))
    }
    #[cfg(target_os = "macos")]
    {
        let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
        // SAFETY: proc_pidinfo writes one aligned proc_bsdinfo record into this buffer.
        let count = unsafe {
            libc::proc_pidinfo(
                pid as i32,
                3,
                0,
                info.as_mut_ptr().cast(),
                std::mem::size_of::<libc::proc_bsdinfo>() as i32,
            )
        };
        if count != std::mem::size_of::<libc::proc_bsdinfo>() as i32 {
            return None;
        }
        // SAFETY: proc_pidinfo returned the complete initialized record.
        let info = unsafe { info.assume_init() };
        if info.pbi_pid != pid || info.pbi_start_tvusec >= 1_000_000 {
            return None;
        }
        let ticks = info
            .pbi_start_tvsec
            .checked_mul(1_000_000)
            .and_then(|seconds| seconds.checked_add(info.pbi_start_tvusec))?;
        Some((ticks, info.pbi_start_tvsec))
    }
    #[cfg(target_os = "windows")]
    {
        let ticks = windows::native_start_time(pid)?;
        let unix_seconds = (ticks / 10_000_000).checked_sub(11_644_473_600)?;
        Some((ticks, unix_seconds))
    }
}

fn send(
    identity: &ProcessIdentity,
    expected_name: Option<&str>,
    signal: ProcessSignal,
) -> Result<(), ActionError> {
    validate_identity(identity)?;
    #[cfg(target_os = "linux")]
    return linux::send(identity, expected_name, signal);
    #[cfg(target_os = "macos")]
    return macos::send(identity, expected_name, signal);
    #[cfg(target_os = "windows")]
    return windows::send(identity, expected_name, signal)
        .map(|_| ())
        .map_err(ActionError::from);
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = signal;
        Err("Identity-safe process actions are not supported on this platform yet".into())
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::{
        fs::File,
        io::Read,
        os::fd::{AsRawFd, FromRawFd, OwnedFd},
    };

    fn error(operation: &str) -> ActionError {
        os_error(operation, std::io::Error::last_os_error())
    }

    pub(super) fn send(
        identity: &ProcessIdentity,
        expected_name: Option<&str>,
        signal: ProcessSignal,
    ) -> Result<(), ActionError> {
        // A pidfd pins the target across exit/PID reuse. Verify start time after
        // acquiring it, then signal the handle rather than the numeric PID.
        // https://man7.org/linux/man-pages/man2/pidfd_send_signal.2.html
        // SAFETY: integer arguments match pidfd_open(2); flags are zero.
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, identity.pid as libc::pid_t, 0_u32) };
        if fd < 0 {
            return Err(error("Open selected process"));
        }
        // SAFETY: a successful pidfd_open returns a new owned descriptor.
        let fd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
        let mut text = String::new();
        File::open(format!("/proc/{}/stat", identity.pid))
            .map_err(|e| os_error("Read selected process identity", e))?
            .take(4096)
            .read_to_string(&mut text)
            .map_err(|e| os_error("Read selected process identity", e))?;
        if text.len() >= 4096 {
            return Err("Selected process identity exceeds its read limit".into());
        }
        if parse_identity(&text)? != *identity {
            return Err(
                "The selected process exited or its PID now belongs to another process".into(),
            );
        }
        if let Some(expected_name) = expected_name
            && process_name(&text)? != expected_name
        {
            return Err("The selected process name no longer matches".into());
        }
        let signal = match signal {
            ProcessSignal::Terminate => libc::SIGTERM,
            ProcessSignal::Kill => libc::SIGKILL,
        };
        // SAFETY: fd remains owned, signal is one of the two supported signals,
        // null siginfo requests kernel-generated metadata, and flags are zero.
        let result = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                fd.as_raw_fd(),
                signal,
                std::ptr::null::<libc::siginfo_t>(),
                0_u32,
            )
        };
        if result < 0 {
            return Err(error("Signal selected process"));
        }
        Ok(())
    }

    pub(super) fn parse_identity(text: &str) -> Result<ProcessIdentity, String> {
        let open = text.find('(').ok_or("Process identity has no name")?;
        let close = text
            .rfind(')')
            .filter(|close| *close > open)
            .ok_or("Process identity has an invalid name")?;
        let pid = text[..open]
            .trim()
            .parse()
            .map_err(|_| "Process identity has an invalid PID")?;
        let start_time_ticks = text[close + 1..]
            .split_whitespace()
            .nth(19)
            .ok_or("Process identity has no start time")?
            .parse()
            .map_err(|_| "Process identity has an invalid start time")?;
        Ok(ProcessIdentity {
            pid,
            start_time_ticks,
        })
    }

    fn process_name(text: &str) -> Result<&str, String> {
        let open = text.find('(').ok_or("Process identity has no name")?;
        let close = text
            .rfind(')')
            .filter(|close| *close > open)
            .ok_or("Process identity has an invalid name")?;
        Ok(&text[open + 1..close])
    }

    #[cfg(test)]
    pub(super) fn boot_time_unix_seconds() -> Option<u64> {
        use std::sync::OnceLock;
        static BOOT_TIME: OnceLock<Option<u64>> = OnceLock::new();
        *BOOT_TIME.get_or_init(|| {
            std::fs::read_to_string("/proc/stat")
                .ok()?
                .lines()
                .find_map(|line| line.strip_prefix("btime ")?.parse().ok())
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn malformed_identity_is_rejected_without_panicking() {
            for text in ["", ")(", "1 ()", "no (name) S", "1 (name) S"] {
                assert!(parse_identity(text).is_err());
            }
            let fields = std::iter::once("S")
                .chain(std::iter::repeat_n("0", 18))
                .chain(["123"])
                .collect::<Vec<_>>()
                .join(" ");
            assert_eq!(
                parse_identity(&format!("42 (odd ) (name) {fields}")),
                Ok(ProcessIdentity {
                    pid: 42,
                    start_time_ticks: 123
                })
            );
            assert_eq!(
                process_name("42 (expected) S 0"),
                Ok::<_, String>("expected")
            );
        }
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;
    use std::{
        os::unix::process::ExitStatusExt,
        process::{Child, Command},
        time::{Duration, Instant},
    };

    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    fn child() -> TestResult<(OwnedChild, ProcessIdentity)> {
        let child = OwnedChild(Command::new("sleep").arg("10").spawn()?);
        #[cfg(target_os = "linux")]
        let stat = std::fs::read_to_string(format!("/proc/{}/stat", child.0.id()))?;
        #[cfg(target_os = "linux")]
        let close = stat.rfind(')').ok_or("missing process name terminator")?;
        #[cfg(target_os = "linux")]
        let start_time_ticks = stat[close + 1..]
            .split_whitespace()
            .nth(19)
            .ok_or("missing process start time")?
            .parse()?;
        #[cfg(target_os = "macos")]
        let start_time_ticks = native_start_time_with_unix_seconds(child.0.id())
            .map(|(ticks, _)| ticks)
            .ok_or("missing process start time")?;
        let identity = ProcessIdentity {
            pid: child.0.id(),
            start_time_ticks,
        };
        Ok((child, identity))
    }

    fn wait(child: &mut Child) -> TestResult<std::process::ExitStatus> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err("signaled child did not exit".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn only_the_matching_owned_identity_receives_each_signal() -> TestResult {
        for (signal, expected) in [(ProcessSignal::Terminate, 15), (ProcessSignal::Kill, 9)] {
            let (mut child, identity) = child()?;
            let wrong = ProcessIdentity {
                start_time_ticks: identity.start_time_ticks + 1,
                ..identity.clone()
            };
            assert!(send_signal(&wrong, signal).is_err());
            assert!(
                child.0.try_wait()?.is_none(),
                "wrong start identity killed child"
            );
            send_signal(&identity, signal).map_err(std::io::Error::other)?;
            assert_eq!(wait(&mut child.0)?.signal(), Some(expected));
        }
        Ok(())
    }

    #[test]
    fn checked_signal_rejects_a_renamed_process_without_signaling_it() -> TestResult {
        let (mut child, identity) = child()?;
        assert!(send_signal_checked(&identity, "not-sleep", ProcessSignal::Terminate).is_err());
        assert!(child.0.try_wait()?.is_none());
        assert!(send_signal_checked(&identity, "sleep", ProcessSignal::Terminate).is_ok());
        assert_eq!(wait(&mut child.0)?.signal(), Some(libc::SIGTERM));
        Ok(())
    }

    #[test]
    fn exited_process_and_protected_targets_are_rejected() -> TestResult {
        let (mut child, identity) = child()?;
        child.0.kill()?;
        child.0.wait()?;
        assert!(send_signal(&identity, ProcessSignal::Kill).is_err());
        for pid in [0, 1, std::process::id(), u32::MAX] {
            assert!(
                send_signal(
                    &ProcessIdentity {
                        pid,
                        start_time_ticks: 1
                    },
                    ProcessSignal::Kill
                )
                .is_err()
            );
        }
        Ok(())
    }
}
