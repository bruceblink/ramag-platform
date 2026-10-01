//! One OS-authorized invocation; no password ever crosses this boundary.
use super::*;
use std::ffi::OsString;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::{
    path::Path,
    process::{Command, Stdio},
};

const HELPER_MODE: &str = "--system-pulse-process-action";

fn with_authentication<T>(
    identity: &ProcessIdentity,
    signal: ProcessSignal,
    ordinary: impl FnOnce(&ProcessIdentity, ProcessSignal) -> Result<T, ActionError>,
    authenticate: impl FnOnce(&ProcessIdentity, ProcessSignal) -> Result<T, String>,
) -> Result<T, String> {
    validate_identity(identity).map_err(|error| error.to_string())?;
    match ordinary(identity, signal) {
        Ok(outcome) => Ok(outcome),
        Err(ActionError::PermissionDenied) => authenticate(identity, signal),
        Err(error) => Err(error.to_string()),
    }
}

/// Must run on a worker: the OS dialog waits for the user's response.
pub fn send_signal_with_authentication(
    identity: &ProcessIdentity,
    signal: ProcessSignal,
) -> Result<ProcessActionOutcome, String> {
    #[cfg(target_os = "windows")]
    return with_authentication(
        identity,
        signal,
        |identity, signal| windows::send(identity, None, signal).map_err(ActionError::from),
        |identity, signal| windows::authenticate(identity, None, signal),
    );
    #[cfg(not(target_os = "windows"))]
    with_authentication(
        identity,
        signal,
        |identity, signal| send(identity, None, signal),
        |identity, signal| authenticate(identity, None, signal),
    )
    .map(|_| ProcessActionOutcome::SignalSent)
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn helper_arguments(
    identity: &ProcessIdentity,
    expected_name: Option<&str>,
    signal: ProcessSignal,
) -> [String; 5] {
    [
        HELPER_MODE.into(),
        identity.pid.to_string(),
        identity.start_time_ticks.to_string(),
        match signal {
            ProcessSignal::Terminate => "terminate",
            ProcessSignal::Kill => "kill",
        }
        .into(),
        encode_name(expected_name),
    ]
}

pub(super) fn encode_name(name: Option<&str>) -> String {
    let Some(name) = name else { return "-".into() };
    let mut encoded = String::with_capacity(name.len() * 2 + 1);
    encoded.push('n');
    for byte in name.bytes() {
        use std::fmt::Write;
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

pub(super) fn decode_name(value: &OsString) -> Result<Option<String>, String> {
    let value = value.to_str().ok_or("Invalid process-action name")?;
    if value == "-" {
        return Ok(None);
    }
    if !value.starts_with('n') || value.len().is_multiple_of(2) || value.len() > 4097 {
        return Err("Invalid process-action name".into());
    }
    let mut bytes = Vec::with_capacity((value.len() - 1) / 2);
    let (pairs, _) = value.as_bytes()[1..].as_chunks::<2>();
    for pair in pairs {
        let hex = std::str::from_utf8(pair).map_err(|_| "Invalid process-action name")?;
        bytes.push(u8::from_str_radix(hex, 16).map_err(|_| "Invalid process-action name")?);
    }
    let name = String::from_utf8(bytes).map_err(|_| "Invalid process-action name")?;
    if name.is_empty() {
        return Err("Invalid process-action name".into());
    }
    Ok(Some(name))
}

pub(super) fn parse_request(
    arguments: &[OsString],
) -> Result<(ProcessIdentity, Option<String>, ProcessSignal), String> {
    if !matches!(arguments.len(), 4 | 5) || arguments[0] != HELPER_MODE {
        return Err("Invalid process-action request".into());
    }
    let number = |index: usize| -> Result<u64, String> {
        let text = arguments[index]
            .to_str()
            .ok_or("Invalid process-action number")?;
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("Invalid process-action number".into());
        }
        text.parse()
            .map_err(|_| "Process-action number is out of range".into())
    };
    let identity = ProcessIdentity {
        pid: u32::try_from(number(1)?).map_err(|_| "Process-action PID is out of range")?,
        start_time_ticks: number(2)?,
    };
    validate_identity(&identity).map_err(|error| error.to_string())?;
    let signal = match arguments[3].to_str() {
        Some("terminate") => ProcessSignal::Terminate,
        Some("kill") => ProcessSignal::Kill,
        _ => return Err("Unsupported process action".into()),
    };
    let expected_name = if arguments.len() == 5 {
        decode_name(&arguments[4])?
    } else {
        None
    };
    Ok((identity, expected_name, signal))
}

/// Call before any GUI or application-state initialization. This entry point
/// performs at most one validated action and never asks for authentication.
pub fn helper_entry(arguments: impl IntoIterator<Item = OsString>) -> Option<i32> {
    let mut arguments = arguments.into_iter();
    let mode = arguments.next()?;
    #[cfg(target_os = "windows")]
    if mode == windows_protocol::HELPER_MODE {
        let request = std::iter::once(mode)
            .chain(arguments.take(7))
            .collect::<Vec<_>>();
        return Some(windows::helper_entry(&request));
    }
    if mode != HELPER_MODE {
        return None;
    }
    // Windows requires its separate mode, caller identity and elevated token.
    #[cfg(target_os = "windows")]
    return Some(13);
    #[cfg(not(target_os = "windows"))]
    {
        let mut request = vec![OsString::from(HELPER_MODE)];
        request.extend(arguments.take(5));
        Some(match parse_request(&request) {
            Ok((identity, expected_name, signal)) => {
                match send(&identity, expected_name.as_deref(), signal) {
                    Ok(()) => 0,
                    Err(ActionError::PermissionDenied) => 11,
                    Err(_) => 12,
                }
            }
            Err(_) => 13,
        })
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn helper_result(code: Option<i32>) -> Result<(), String> {
    match code {
        Some(0) => Ok(()),
        Some(11) => Err("The operating system denied this action even with administrative permission. The process was not changed.".into()),
        Some(12) => Err("The selected process exited, changed identity, or could not be safely signaled. The process was not changed.".into()),
        Some(13) => Err("The authenticated process-action request was invalid. The process was not changed.".into()),
        Some(126) => Err("Authentication was cancelled. The process was not changed.".into()),
        Some(127) => Err("Authentication failed or no system authentication agent is available. The process was not changed.".into()),
        Some(code) if code < 0 => Err(format!("macOS authentication failed (error {code}). The process was not changed.")),
        _ => Err("The system process-action helper did not complete. Check the process list before trying again.".into()),
    }
}

#[cfg(target_os = "linux")]
fn authentication_command(
    executable: &Path,
    identity: &ProcessIdentity,
    expected_name: Option<&str>,
    signal: ProcessSignal,
) -> Command {
    let mut command = Command::new("/usr/bin/pkexec");
    command
        .arg("--disable-internal-agent")
        .arg(executable)
        .args(helper_arguments(identity, expected_name, signal));
    command
}

#[cfg(target_os = "macos")]
const AUTHENTICATE_SCRIPT: &str = r#"on run argv
    set commandText to ""
    repeat with argument in argv
        set commandText to commandText & quoted form of (argument as text) & " "
    end repeat
    try
        return do shell script (commandText & "; result=$?; /usr/bin/printf '%s' \"$result\"") with administrator privileges
    on error messageText number errorNumber
        if errorNumber is -128 then return "126"
        return errorNumber as text
    end try
end run"#;

#[cfg(target_os = "macos")]
fn authentication_command(
    executable: &Path,
    identity: &ProcessIdentity,
    expected_name: Option<&str>,
    signal: ProcessSignal,
) -> Command {
    let mut command = Command::new("/usr/bin/osascript");
    command
        .args(["-e", AUTHENTICATE_SCRIPT, "--"])
        .arg(executable)
        .args(helper_arguments(identity, expected_name, signal));
    command
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn authenticate(
    identity: &ProcessIdentity,
    expected_name: Option<&str>,
    signal: ProcessSignal,
) -> Result<(), String> {
    let executable = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map_err(|error| format!("Locate the process-action helper: {error}"))?;
    let mut command = authentication_command(&executable, identity, expected_name, signal);
    // No terminal password fallback and no inherited diagnostic streams.
    command.stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(target_os = "linux")]
    {
        let status = command.stdout(Stdio::null()).status()
            .map_err(|error| format!("Start system authentication (install polkit and a desktop authentication agent): {error}"))?;
        helper_result(status.code())
    }
    #[cfg(target_os = "macos")]
    {
        let output = command
            .output()
            .map_err(|error| format!("Start system authentication: {error}"))?;
        if !output.status.success() {
            return Err("System authentication did not complete. Check the process list before trying again.".into());
        }
        helper_result(
            std::str::from_utf8(&output.stdout)
                .ok()
                .and_then(|text| text.trim().parse().ok()),
        )
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn authenticate(_: &ProcessIdentity, _: Option<&str>, _: ProcessSignal) -> Result<(), String> {
    Err("System authentication is not supported on this platform".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> ProcessIdentity {
        ProcessIdentity {
            pid: 4242,
            start_time_ticks: 987654321,
        }
    }

    #[test]
    fn only_permission_denial_authenticates_and_preserves_the_request() {
        for signal in [ProcessSignal::Terminate, ProcessSignal::Kill] {
            let mut authenticated = false;
            let ordinary_success = with_authentication(
                &identity(),
                signal,
                |_, _| Ok(()),
                |_, _| {
                    authenticated = true;
                    Err("unexpected authentication".into())
                },
            );
            assert_eq!(ordinary_success, Ok(()));
            assert!(!authenticated);
            let identity_failure = with_authentication::<()>(
                &identity(),
                signal,
                |_, _| Err("identity changed".into()),
                |_, _| {
                    authenticated = true;
                    Err("unexpected authentication".into())
                },
            );
            assert_eq!(identity_failure, Err("identity changed".into()));
            assert!(!authenticated);
            let result = with_authentication::<()>(
                &identity(),
                signal,
                |_, _| Err(ActionError::PermissionDenied),
                |request, action| {
                    assert_eq!(*request, identity());
                    assert_eq!(action, signal);
                    Err("cancelled".into())
                },
            );
            assert_eq!(result, Err("cancelled".into()));
        }
    }

    #[test]
    fn invalid_identity_never_dispatches_or_authenticates() {
        for request in [
            ProcessIdentity {
                pid: 1,
                ..identity()
            },
            ProcessIdentity {
                start_time_ticks: 0,
                ..identity()
            },
        ] {
            assert!(
                with_authentication::<()>(
                    &request,
                    ProcessSignal::Kill,
                    |_, _| Err("invalid request reached the operating system".into()),
                    |_, _| Err("invalid request requested authorization".into())
                )
                .is_err()
            );
        }
    }

    #[test]
    fn helper_rejects_extra_missing_overflow_and_shell_input() {
        let valid = helper_arguments(&identity(), None, ProcessSignal::Kill).map(OsString::from);
        assert_eq!(
            parse_request(&valid),
            Ok((identity(), None, ProcessSignal::Kill))
        );
        for (index, bad) in [
            (1, "0"),
            (1, "-1"),
            (1, "4294967296"),
            (1, "42;kill -9 1"),
            (2, "0"),
            (2, "18446744073709551616"),
            (3, "stop"),
        ] {
            let mut request = valid.clone();
            request[index] = bad.into();
            assert!(parse_request(&request).is_err());
        }
        assert!(parse_request(&valid[..3]).is_err());
        let mut extra = valid.to_vec();
        extra.push("extra".into());
        assert_eq!(helper_entry(extra), Some(13));
        assert_eq!(helper_entry([OsString::from("--unrelated")]), None);
    }

    #[test]
    fn cancellation_and_helper_failures_are_not_success() {
        assert!(helper_result(Some(0)).is_ok());
        for code in [
            Some(11),
            Some(12),
            Some(13),
            Some(126),
            Some(127),
            Some(1),
            None,
        ] {
            assert!(helper_result(code).is_err());
        }
        let cancellation = helper_result(Some(126));
        assert!(matches!(cancellation, Err(error) if error.contains("cancelled")));
    }

    #[test]
    fn native_authentication_error_retains_only_its_numeric_context() {
        assert_eq!(
            helper_result(Some(-60007)),
            Err("macOS authentication failed (error -60007). The process was not changed.".into())
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn applescript_reports_error_number_without_error_message()
    -> Result<(), Box<dyn std::error::Error>> {
        let script = AUTHENTICATE_SCRIPT.replace(
            "    try\n",
            "    try\n        error \"private error details\" number -60007\n",
        );
        let output = Command::new("/usr/bin/osascript")
            .args(["-e", &script, "--"])
            .output()?;
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout)?.trim(), "-60007");
        assert!(output.stderr.is_empty());
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn authentication_uses_fixed_program_and_separate_arguments() {
        let command = authentication_command(
            Path::new("/tmp/path with ' quotes/$literal/app"),
            &identity(),
            None,
            ProcessSignal::Terminate,
        );
        assert_eq!(command.get_program(), "/usr/bin/pkexec");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                "--disable-internal-agent",
                "/tmp/path with ' quotes/$literal/app",
                HELPER_MODE,
                "4242",
                "987654321",
                "terminate",
                "-"
            ]
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn applescript_preserves_arguments_and_returns_helper_exit_status()
    -> Result<(), Box<dyn std::error::Error>> {
        // Exercise Apple's real parser and shell quoting without requesting
        // privileges. The production script differs only by the OS auth clause.
        let directory =
            std::env::temp_dir().join(format!("pulse-action-'-$literal-{}", std::process::id()));
        std::fs::create_dir_all(&directory)?;
        let executable = directory.join("false helper");
        std::os::unix::fs::symlink("/usr/bin/false", &executable)?;
        let output = Command::new("/usr/bin/osascript")
            .args([
                "-e",
                &AUTHENTICATE_SCRIPT.replace(" with administrator privileges", ""),
                "--",
            ])
            .arg(&executable)
            .args(helper_arguments(&identity(), None, ProcessSignal::Kill))
            .output()?;
        std::fs::remove_file(&executable)?;
        std::fs::remove_dir(&directory)?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout)?.trim(), "1");
        Ok(())
    }
}
