use super::*;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{error::Error, fmt::Debug, io};

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn checked<T, E: Debug>(result: Result<T, E>) -> TestResult<T> {
    result.map_err(|error| io::Error::other(format!("{error:?}")).into())
}

// Inspect native window styles and alpha so the test verifies actual transparency.
fn window_state(pid: u32) -> TestResult<(usize, usize, usize)> {
    struct Probe {
        pid: u32,
        windows: usize,
        visible: usize,
        transparent: usize,
    }
    unsafe extern "system" fn visit(window: HWND, context: LPARAM) -> BOOL {
        // EnumWindows calls synchronously with this stack-owned probe.
        let probe = unsafe { &mut *(context.0 as *mut Probe) };
        let mut owner = 0;
        unsafe { GetWindowThreadProcessId(window, Some(&mut owner)) };
        if owner == probe.pid {
            probe.windows += 1;
            use ::windows::Win32::UI::WindowsAndMessaging::{
                GWL_EXSTYLE, GetLayeredWindowAttributes, GetWindowLongPtrW, IsWindowVisible,
                WS_EX_LAYERED,
            };
            let visible = unsafe { IsWindowVisible(window).as_bool() };
            if visible {
                probe.visible += 1;
            }
            let extended_style = unsafe { GetWindowLongPtrW(window, GWL_EXSTYLE) };
            if visible && extended_style & WS_EX_LAYERED.0 as isize != 0 {
                let mut alpha = u8::MAX;
                if unsafe { GetLayeredWindowAttributes(window, None, Some(&mut alpha), None) }
                    .is_ok()
                    && alpha == 0
                {
                    probe.transparent += 1;
                }
            }
        }
        BOOL(1)
    }

    let mut probe = Probe {
        pid,
        windows: 0,
        visible: 0,
        transparent: 0,
    };
    checked(unsafe { EnumWindows(Some(visit), LPARAM((&mut probe as *mut Probe) as isize)) })?;
    Ok((probe.windows, probe.visible, probe.transparent))
}

struct TempDirectory(PathBuf);
impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn child() -> TestResult<(OwnedChild, ProcessIdentity)> {
    let executable = PathBuf::from(
        std::env::var_os("WINDIR").ok_or_else(|| io::Error::other("WINDIR is unavailable"))?,
    )
    .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let child = OwnedChild(
        Command::new(executable)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 60",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?,
    );
    let identity = ProcessIdentity {
        pid: child.0.id(),
        start_time_ticks: checked(creation_time(HANDLE(child.0.as_raw_handle())))?,
    };
    Ok((child, identity))
}

#[test]
fn denied_safety_verification_does_not_request_authorization() {
    let denied = verification_error(Failure::PermissionDenied);
    assert!(matches!(
        super::super::ActionError::from(denied),
        super::super::ActionError::Failed(_)
    ));
    for error in [
        Failure::Protected,
        Failure::IdentityChanged,
        Failure::TargetExited,
    ] {
        assert_eq!(verification_error(error.clone()), error);
    }
    assert_eq!(
        super::super::ActionError::from(Failure::PermissionDenied),
        super::super::ActionError::PermissionDenied
    );
}

#[test]
fn native_collection_preserves_all_creation_ticks_and_rejects_one_tick_mismatch() -> TestResult {
    let (mut child, identity) = child()?;
    assert_eq!(
        native_start_time(identity.pid),
        Some(identity.start_time_ticks)
    );
    assert!(identity.start_time_ticks > 100_000_000_000_000_000);
    for ticks in [
        0,
        identity.start_time_ticks - 1,
        identity.start_time_ticks + 1,
    ] {
        let wrong = ProcessIdentity {
            start_time_ticks: ticks,
            ..identity.clone()
        };
        assert!(send(&wrong, None, ProcessSignal::Kill).is_err());
        assert!(
            child.0.try_wait()?.is_none(),
            "wrong identity changed owned child"
        );
    }
    assert_eq!(
        send(&identity, None, ProcessSignal::Kill),
        Ok(ProcessActionOutcome::ExitObserved)
    );
    assert!(child.0.try_wait()?.is_some());
    Ok(())
}

#[test]
fn native_exited_and_protected_targets_are_refused() -> TestResult {
    let (mut child, identity) = child()?;
    child.0.kill()?;
    child.0.wait()?;
    assert_eq!(
        send(&identity, None, ProcessSignal::Kill),
        Err(Failure::TargetExited)
    );
    for pid in [0, 1, 4, std::process::id(), u32::MAX] {
        assert!(
            send(
                &ProcessIdentity {
                    pid,
                    start_time_ticks: 1
                },
                None,
                ProcessSignal::Kill
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn checked_action_requires_the_current_process_name() -> TestResult {
    let (mut child, identity) = child()?;
    assert!(
        super::super::send_signal_checked(&identity, "wrong.exe", ProcessSignal::Kill).is_err()
    );
    assert!(child.0.try_wait()?.is_none());
    assert!(
        super::super::send_signal_checked(&identity, "powershell.exe", ProcessSignal::Kill).is_ok()
    );
    assert!(child.0.try_wait()?.is_some());
    Ok(())
}

#[test]
fn native_pinned_process_and_file_identity_survive_path_aliases() -> TestResult {
    let (child, identity) = child()?;
    let process = checked(VerifiedProcess::open(
        &identity,
        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE | PROCESS_TERMINATE,
    ))?;
    checked(process.protect_target())?;
    assert_eq!(
        checked(creation_time(raw(&process.handle)))?,
        identity.start_time_ticks
    );
    assert!(!checked(process.is_application())?);
    assert_eq!(unsafe { GetProcessId(raw(&process.handle)) }, child.0.id());
    let executable = std::env::current_exe()?;
    assert_eq!(
        checked(executable_identity(&executable))?,
        checked(executable_identity(&std::fs::canonicalize(executable)?))?
    );
    Ok(())
}

#[test]
fn native_graceful_close_honors_refusal_and_never_messages_an_unrelated_process() -> TestResult {
    let directory = TempDirectory(std::env::temp_dir().join(format!(
        "pulse-owned-gui-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    )));
    std::fs::create_dir(&directory.0)?;
    let source = directory.0.join("fixture.cs");
    std::fs::write(
        &source,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/windows_process_fixture.cs"
        )),
    )?;
    let executable = directory.0.join("process fixture.exe");
    let compiler = PathBuf::from(
        std::env::var_os("WINDIR").ok_or_else(|| io::Error::other("WINDIR is unavailable"))?,
    )
    .join("Microsoft.NET/Framework64/v4.0.30319/csc.exe");
    let status = Command::new(compiler)
        .args([
            "/nologo",
            "/target:winexe",
            "/reference:System.Windows.Forms.dll",
        ])
        .arg(format!("/out:{}", executable.display()))
        .arg(&source)
        .status()?;
    assert!(status.success(), "compile disposable GUI fixture");
    let start = |mode: &str, label: &str| -> TestResult<_> {
        let log = directory.0.join(format!("{label}.log"));
        let child = OwnedChild(Command::new(&executable).arg(mode).arg(&log).spawn()?);
        let deadline = Instant::now() + Duration::from_secs(15);
        while !log.exists() {
            assert!(
                Instant::now() < deadline,
                "GUI fixture did not become ready"
            );
            std::thread::sleep(Duration::from_millis(30));
        }
        let identity = ProcessIdentity {
            pid: child.0.id(),
            start_time_ticks: checked(creation_time(HANDLE(child.0.as_raw_handle())))?,
        };
        Ok((child, identity, log))
    };
    {
        let (mut control, _, control_log) = start("cooperative", "control")?;
        let (control_windows, control_visible, control_transparent) = window_state(control.0.id())?;
        assert!(control_windows > 0);
        assert!(control_visible > 0);
        assert_eq!(control_transparent, control_visible);
        for (mode, expected) in [
            ("cooperative", Ok(ProcessActionOutcome::ExitObserved)),
            ("refusing", Err(Failure::CloseRefused)),
            ("windowless", Err(Failure::GracefulUnavailable)),
        ] {
            let (mut target, identity, log) = start(mode, mode)?;
            let wrong = ProcessIdentity {
                start_time_ticks: identity.start_time_ticks + 1,
                ..identity.clone()
            };
            assert_eq!(
                send(&wrong, None, ProcessSignal::Terminate),
                Err(Failure::IdentityChanged)
            );
            assert!(target.0.try_wait()?.is_none());
            let (windows, visible, transparent) = window_state(target.0.id())?;
            if mode == "windowless" {
                assert_eq!(windows, 0);
            } else {
                assert!(windows > 0, "GUI fixture must own a top-level window");
                assert!(visible > 0, "GUI fixture must retain window visibility");
                assert_eq!(
                    transparent, visible,
                    "every shown window must have zero alpha"
                );
            }
            assert_eq!(
                std::fs::read_to_string(&log)?,
                if mode == "windowless" {
                    "ready\n"
                } else {
                    "ready:transparent\n"
                }
            );
            assert_eq!(send(&identity, None, ProcessSignal::Terminate), expected);
            assert!(control.0.try_wait()?.is_none());
            assert_eq!(
                std::fs::read_to_string(&control_log)?,
                "ready:transparent\n"
            );
            if mode != "cooperative" {
                assert!(target.0.try_wait()?.is_none());
            }
            if mode == "refusing" {
                let (windows, visible, transparent) = window_state(target.0.id())?;
                assert!(windows > 0, "refusing fixture must keep its window");
                assert!(visible > 0, "refusal must preserve native window behavior");
                assert_eq!(
                    transparent, visible,
                    "refusal must not expose the fixture window"
                );
            }
        }
    }
    Ok(())
}
