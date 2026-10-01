//! Windows process actions keep an independently verified process object alive.
use super::windows_protocol::{self, Failure, Request};
use super::{ProcessActionOutcome, ProcessIdentity, ProcessSignal};
use ::windows::{
    Win32::{
        Foundation::{
            BOOL, ERROR_ACCESS_DENIED, ERROR_FAIL_SHUTDOWN, ERROR_INVALID_PARAMETER, ERROR_SUCCESS,
            FILETIME, HANDLE, HWND, LPARAM, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT, WIN32_ERROR,
        },
        Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation},
        Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, FILE_READ_ATTRIBUTES, GetFileInformationByHandle,
        },
        System::{
            RestartManager::{
                CCH_RM_SESSION_KEY, RM_PROCESS_INFO, RM_UNIQUE_PROCESS, RmCritical, RmEndSession,
                RmGetList, RmMainWindow, RmRebootReasonPermissionDenied, RmRegisterResources,
                RmShutdown, RmStartSession,
            },
            Threading::{
                GetCurrentProcess, GetExitCodeProcess, GetProcessId, GetProcessInformation,
                GetProcessTimes, IsProcessCritical, OpenProcess, OpenProcessToken,
                PROCESS_ACCESS_RIGHTS, PROCESS_NAME_WIN32, PROCESS_PROTECTION_LEVEL_INFORMATION,
                PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
                PROTECTION_LEVEL_NONE, ProcessProtectionLevelInfo, QueryFullProcessImageNameW,
                TerminateProcess, WaitForSingleObject,
            },
        },
        UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId},
    },
    core::{PCWSTR, PWSTR},
};
use std::{
    ffi::OsString,
    fs::OpenOptions,
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::PathBuf,
};

const OBSERVE_EXIT_MS: u32 = 1_500;
const HELPER_WAIT_MS: u32 = 120_000;
mod authorization;

fn native_error(operation: &'static str, error: ::windows::core::Error) -> Failure {
    native_code(operation, WIN32_ERROR(error.code().0 as u32 & 0xffff))
}

fn native_code(operation: &'static str, code: WIN32_ERROR) -> Failure {
    if code == ERROR_ACCESS_DENIED {
        Failure::PermissionDenied
    } else {
        Failure::OperatingSystem {
            operation,
            code: code.0,
        }
    }
}

fn raw(handle: &OwnedHandle) -> HANDLE {
    HANDLE(handle.as_raw_handle())
}

fn filetime(value: FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}

fn creation_time(handle: HANDLE) -> Result<u64, Failure> {
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // SAFETY: the caller retains the process handle; all output pointers are distinct and valid.
    unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) }
        .map_err(|e| native_error("Read process creation time", e))?;
    let ticks = filetime(created);
    if ticks == 0 {
        Err(Failure::IdentityChanged)
    } else {
        Ok(ticks)
    }
}

pub(super) fn native_start_time(pid: u32) -> Option<u64> {
    // SAFETY: query-only access is sufficient to read the creation FILETIME.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()? };
    // SAFETY: OpenProcess returned an owned process handle that must be closed once.
    let handle = unsafe { OwnedHandle::from_raw_handle(handle.0 as _) };
    creation_time(raw(&handle)).ok()
}

struct VerifiedProcess {
    handle: OwnedHandle,
    identity: ProcessIdentity,
}

impl VerifiedProcess {
    fn open(identity: &ProcessIdentity, rights: PROCESS_ACCESS_RIGHTS) -> Result<Self, Failure> {
        super::validate_identity(identity).map_err(|_| Failure::InvalidRequest)?;
        if identity.pid == 4 {
            return Err(Failure::Protected);
        }
        // SAFETY: the PID and access mask are values; inheritance is disabled.
        let handle = unsafe { OpenProcess(rights, false, identity.pid) }.map_err(|e| {
            let code = WIN32_ERROR(e.code().0 as u32 & 0xffff);
            if code == ERROR_INVALID_PARAMETER {
                Failure::TargetExited
            } else {
                native_code("Open selected process", code)
            }
        })?;
        // SAFETY: OpenProcess returned one new owned, non-null kernel handle.
        let handle = unsafe { OwnedHandle::from_raw_handle(handle.0) };
        // GetProcessId also rejects numeric PID aliases accepted by some kernels.
        if unsafe { GetProcessId(raw(&handle)) } != identity.pid
            || creation_time(raw(&handle))? != identity.start_time_ticks
        {
            return Err(Failure::IdentityChanged);
        }
        let process = Self {
            handle,
            identity: identity.clone(),
        };
        if process.exited(0)? {
            return Err(Failure::TargetExited);
        }
        Ok(process)
    }

    fn exited(&self, timeout: u32) -> Result<bool, Failure> {
        // SAFETY: this object owns a handle opened with SYNCHRONIZE throughout the wait.
        match unsafe { WaitForSingleObject(raw(&self.handle), timeout) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            WAIT_FAILED => Err(native_error(
                "Observe process exit",
                ::windows::core::Error::from_win32(),
            )),
            _ => Err(Failure::UnknownOutcome),
        }
    }

    fn verify_name(&self, expected: &str) -> Result<(), Failure> {
        let mut buffer = vec![0u16; 32_768];
        let mut length = buffer.len() as u32;
        // SAFETY: the buffer has the declared writable capacity and handle pins the process.
        unsafe {
            QueryFullProcessImageNameW(
                raw(&self.handle),
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut length,
            )
        }
        .map_err(|e| native_error("Read selected process name", e))?;
        if length == 0 || length as usize >= buffer.len() {
            return Err(Failure::IdentityChanged);
        }
        let path = PathBuf::from(OsString::from_wide(&buffer[..length as usize]));
        let actual = path
            .file_name()
            .ok_or(Failure::IdentityChanged)?
            .to_string_lossy();
        if actual.eq_ignore_ascii_case(expected) {
            Ok(())
        } else {
            Err(Failure::IdentityChanged)
        }
    }

    fn protect_target(&self) -> Result<(), Failure> {
        let mut critical = BOOL::default();
        // SAFETY: the retained process handle and output structure remain valid.
        unsafe { IsProcessCritical(raw(&self.handle), &mut critical) }
            .map_err(|e| native_error("Check critical process protection", e))?;
        let mut protection = PROCESS_PROTECTION_LEVEL_INFORMATION::default();
        unsafe {
            GetProcessInformation(
                raw(&self.handle),
                ProcessProtectionLevelInfo,
                (&mut protection as *mut PROCESS_PROTECTION_LEVEL_INFORMATION).cast(),
                std::mem::size_of_val(&protection) as u32,
            )
        }
        .map_err(|e| native_error("Check process protection level", e))?;
        if critical.as_bool()
            || protection.ProtectionLevel != PROTECTION_LEVEL_NONE
            || self.is_application()?
        {
            return Err(Failure::Protected);
        }
        Ok(())
    }

    fn is_application(&self) -> Result<bool, Failure> {
        let mut buffer = vec![0u16; 32_768];
        let mut length = buffer.len() as u32;
        // SAFETY: the mutable buffer has the declared capacity and the handle remains owned.
        unsafe {
            QueryFullProcessImageNameW(
                raw(&self.handle),
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut length,
            )
        }
        .map_err(|e| native_error("Read process executable identity", e))?;
        if length == 0 || length as usize >= buffer.len() {
            return Err(Failure::InvalidRequest);
        }
        let target = PathBuf::from(OsString::from_wide(&buffer[..length as usize]));
        let current = std::env::current_exe().map_err(|e| Failure::OperatingSystem {
            operation: "Locate application executable",
            code: e.raw_os_error().unwrap_or(0) as u32,
        })?;
        Ok(executable_identity(&target)? == executable_identity(&current)?)
    }

    fn terminate(&self) -> Result<ProcessActionOutcome, Failure> {
        // SAFETY: this is the same identity-checked handle, opened with PROCESS_TERMINATE.
        let result = unsafe { TerminateProcess(raw(&self.handle), 1) };
        if let Err(error) = result {
            if self.exited(0)? {
                return Ok(ProcessActionOutcome::ExitObserved);
            }
            return Err(native_error("Terminate selected process", error));
        }
        if self.exited(OBSERVE_EXIT_MS)? {
            Ok(ProcessActionOutcome::ExitObserved)
        } else {
            Ok(ProcessActionOutcome::TerminationPending)
        }
    }

    fn close(&self) -> Result<ProcessActionOutcome, Failure> {
        // Window discovery is only a support check, never an action destination.
        // A background target must not prompt merely because termination needs
        // permission. Restart Manager still validates the full identity below.
        if !self.has_window()? {
            if self.exited(0)? {
                return Ok(ProcessActionOutcome::ExitObserved);
            }
            return Err(Failure::GracefulUnavailable);
        }
        let session = RestartSession::new()?;
        let result = self.close_in_session(&session);
        // End the private registration even when the action was refused.
        session.finish()?;
        result
    }

    fn has_window(&self) -> Result<bool, Failure> {
        struct Probe {
            pid: u32,
            found: bool,
        }
        unsafe extern "system" fn visit(window: HWND, context: LPARAM) -> BOOL {
            // SAFETY: EnumWindows synchronously borrows the stack-owned Probe.
            let probe = unsafe { &mut *(context.0 as *mut Probe) };
            let mut owner = 0;
            unsafe { GetWindowThreadProcessId(window, Some(&mut owner)) };
            if owner == probe.pid {
                probe.found = true;
                BOOL(0)
            } else {
                BOOL(1)
            }
        }
        let mut probe = Probe {
            pid: self.identity.pid,
            found: false,
        };
        // The retained process handle pins its PID. No HWND escapes this call.
        let result =
            unsafe { EnumWindows(Some(visit), LPARAM((&mut probe as *mut Probe) as isize)) };
        if probe.found {
            Ok(true)
        } else {
            result.map_err(|e| Failure::OperatingSystem {
                operation: "Discover selected application windows",
                code: e.code().0 as u32 & 0xffff,
            })?;
            Ok(false)
        }
    }

    fn close_in_session(&self, session: &RestartSession) -> Result<ProcessActionOutcome, Failure> {
        let ticks = self.identity.start_time_ticks;
        let application = RM_UNIQUE_PROCESS {
            dwProcessId: self.identity.pid,
            ProcessStartTime: FILETIME {
                dwLowDateTime: ticks as u32,
                dwHighDateTime: (ticks >> 32) as u32,
            },
        };
        // Register only this process, never filenames or services that expand the affected set.
        // SAFETY: all inputs are initialized; the session and process handle stay live.
        let code = unsafe { RmRegisterResources(session.id()?, None, Some(&[application]), None) };
        if code != ERROR_SUCCESS {
            return Err(native_code("Register selected GUI process", code));
        }
        let (mut needed, mut count, mut reasons) = (0, 1, 0);
        let mut affected = RM_PROCESS_INFO::default();
        let code = unsafe {
            RmGetList(
                session.id()?,
                &mut needed,
                &mut count,
                Some(&mut affected),
                &mut reasons,
            )
        };
        if self.exited(0)? {
            return Ok(ProcessActionOutcome::ExitObserved);
        }
        if code != ERROR_SUCCESS || count != 1 || needed > 1 {
            return Err(Failure::GracefulUnavailable);
        }
        if affected.Process.dwProcessId != self.identity.pid
            || filetime(affected.Process.ProcessStartTime) != ticks
        {
            return Err(Failure::IdentityChanged);
        }
        if reasons == RmRebootReasonPermissionDenied.0 as u32
            && affected.ApplicationType == RmCritical
        {
            return Err(Failure::PermissionDenied);
        }
        if reasons != 0
            || affected.ApplicationType != RmMainWindow
            || affected.strServiceShortName[0] != 0
        {
            return Err(Failure::GracefulUnavailable);
        }
        // Our retained handle prevents PID reuse even after exit. Restart Manager receives
        // a process registration, not a cached HWND. Flags zero never force refusal to exit.
        // SAFETY: the private session and verified process remain owned through the call.
        let code = unsafe { RmShutdown(session.id()?, 0, None) };
        if self.exited(OBSERVE_EXIT_MS)? {
            return Ok(ProcessActionOutcome::ExitObserved);
        }
        match code {
            ERROR_SUCCESS => Ok(ProcessActionOutcome::CloseRequested),
            ERROR_FAIL_SHUTDOWN => Err(Failure::CloseRefused),
            // A failure after shutdown started may have effects. Never elevate/retry it.
            _ => Err(Failure::OperatingSystem {
                operation: "Request application closure",
                code: code.0,
            }),
        }
    }
}

fn executable_identity(path: &std::path::Path) -> Result<(u32, u64), Failure> {
    let file = OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES.0)
        .open(path)
        .map_err(|e| {
            native_code(
                "Read executable file identity",
                WIN32_ERROR(e.raw_os_error().unwrap_or(0) as u32),
            )
        })?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: file owns a readable-attributes handle and info is an initialized output.
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info) }
        .map_err(|e| native_error("Compare executable file identity", e))?;
    Ok((
        info.dwVolumeSerialNumber,
        (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    ))
}

struct RestartSession(Option<u32>);
impl RestartSession {
    fn new() -> Result<Self, Failure> {
        let mut id = 0;
        let mut key = vec![0u16; CCH_RM_SESSION_KEY as usize + 1];
        // SAFETY: outputs have the documented sizes; flags zero create a private session.
        let code = unsafe { RmStartSession(&mut id, 0, PWSTR(key.as_mut_ptr())) };
        if code == ERROR_SUCCESS {
            Ok(Self(Some(id)))
        } else {
            Err(native_code("Start graceful-close session", code))
        }
    }
    fn id(&self) -> Result<u32, Failure> {
        self.0.ok_or(Failure::UnknownOutcome)
    }
    fn finish(mut self) -> Result<(), Failure> {
        let id = self.0.take().ok_or(Failure::UnknownOutcome)?;
        // SAFETY: this object owns the session and ends it exactly once.
        let code = unsafe { RmEndSession(id) };
        if code == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(Failure::OperatingSystem {
                operation: "End graceful-close session",
                code: code.0,
            })
        }
    }
}
impl Drop for RestartSession {
    fn drop(&mut self) {
        if let Some(id) = self.0.take() {
            // SAFETY: unwind-only fallback for this object's owned session.
            let _ = unsafe { RmEndSession(id) };
        }
    }
}

pub(super) fn send(
    identity: &ProcessIdentity,
    expected_name: Option<&str>,
    signal: ProcessSignal,
) -> Result<ProcessActionOutcome, Failure> {
    let mut rights = PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE;
    if signal == ProcessSignal::Kill {
        rights |= PROCESS_TERMINATE;
    }
    let process = match VerifiedProcess::open(identity, rights) {
        Err(Failure::PermissionDenied) if signal == ProcessSignal::Kill => {
            // Failure to obtain termination rights says nothing about whether
            // this is a protected or stale target. Verify those facts through a
            // read-only handle before offering authorization. Never reopen by
            // PID to act through this handle; the helper verifies its own handle.
            let candidate = VerifiedProcess::open(
                identity,
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            )
            .map_err(verification_error)?;
            candidate.protect_target().map_err(verification_error)?;
            return Err(Failure::PermissionDenied);
        }
        result => result.map_err(verification_error)?,
    };
    if let Some(expected_name) = expected_name {
        process
            .verify_name(expected_name)
            .map_err(verification_error)?;
    }
    process.protect_target().map_err(verification_error)?;
    match signal {
        ProcessSignal::Kill => process.terminate(),
        ProcessSignal::Terminate => process.close(),
    }
}

fn verification_error(error: Failure) -> Failure {
    match error {
        Failure::PermissionDenied => Failure::OperatingSystem {
            operation: "Verify that the selected process is safe to control",
            code: ERROR_ACCESS_DENIED.0,
        },
        error => error,
    }
}

fn elevated() -> Result<bool, Failure> {
    let mut token = HANDLE::default();
    // SAFETY: GetCurrentProcess is a borrowed pseudo handle; OpenProcessToken returns a new handle.
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }
        .map_err(|e| native_error("Read current process token", e))?;
    let token = unsafe { OwnedHandle::from_raw_handle(token.0) };
    let mut elevation = TOKEN_ELEVATION::default();
    let mut length = 0;
    unsafe {
        GetTokenInformation(
            raw(&token),
            TokenElevation,
            Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
            std::mem::size_of_val(&elevation) as u32,
            &mut length,
        )
    }
    .map_err(|e| native_error("Read token elevation", e))?;
    Ok(elevation.TokenIsElevated != 0)
}

pub(super) fn helper_entry(arguments: &[OsString]) -> i32 {
    windows_protocol::encode_result((|| {
        let request = Request::parse(arguments)?;
        if !elevated()? {
            return Err(Failure::InvalidRequest);
        }
        let caller = VerifiedProcess::open(
            &request.caller,
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
        )?;
        if !caller.is_application()? {
            return Err(Failure::InvalidRequest);
        }
        // Keep the caller pinned as well; it cannot become a different process during the action.
        send(
            &request.target,
            request.expected_name.as_deref(),
            request.signal,
        )
    })())
}

pub(super) fn authenticate(
    identity: &ProcessIdentity,
    expected_name: Option<&str>,
    signal: ProcessSignal,
) -> Result<ProcessActionOutcome, String> {
    let result = (|| {
        if elevated()? {
            return Err(Failure::PermissionDenied);
        }
        let caller = ProcessIdentity {
            pid: std::process::id(),
            start_time_ticks: creation_time(unsafe { GetCurrentProcess() })?,
        };
        let request = Request {
            target: identity.clone(),
            expected_name: expected_name.map(str::to_owned),
            signal,
            caller,
        };
        // A dedicated STA avoids inheriting an unrelated apartment from a reused pool thread.
        std::thread::spawn(move || authorization::authenticate_in_apartment(request))
            .join()
            .unwrap_or(Err(Failure::UnknownOutcome))
    })();
    result.map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "../windows/tests.rs"]
mod tests;
