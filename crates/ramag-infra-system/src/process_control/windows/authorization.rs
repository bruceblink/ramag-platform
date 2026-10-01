use super::*;
use ::windows::Win32::{
    Foundation::{ERROR_CANCELLED, WIN32_ERROR},
    System::Com::{
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
    },
    UI::{
        Shell::{
            SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
            ShellExecuteExW,
        },
        WindowsAndMessaging::SW_HIDE,
    },
};
use ::windows::core::w;

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

pub(super) fn authenticate_in_apartment(request: Request) -> Result<ProcessActionOutcome, Failure> {
    // SAFETY: this is a fresh owned thread, paired with CoUninitialize on every return.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }
        .ok()
        .map_err(|error| native_error("Initialize Windows authorization", error))?;
    let _apartment = Apartment;
    let path = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map_err(|error| {
            native_code(
                "Locate authorization helper",
                WIN32_ERROR(error.raw_os_error().unwrap_or(0) as u32),
            )
        })?;
    if !path.is_absolute() {
        return Err(Failure::InvalidRequest);
    }
    let mut executable = path.as_os_str().encode_wide().collect::<Vec<_>>();
    // Rust canonical paths use the extended prefix; the shell expects the absolute DOS/UNC form.
    if executable.starts_with(&[92, 92, 63, 92, 85, 78, 67, 92]) {
        executable.splice(..8, [92, 92]);
    } else if executable.starts_with(&[92, 92, 63, 92]) {
        executable.drain(..4);
    }
    if executable.contains(&0) {
        return Err(Failure::InvalidRequest);
    }
    executable.push(0);
    let parameters = request
        .parameters()
        .encode_utf16()
        .chain([0])
        .collect::<Vec<_>>();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(executable.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // SAFETY: all UTF-16 strings are NUL-terminated and remain live for the synchronous launch.
    if let Err(error) = unsafe { ShellExecuteExW(&mut info) } {
        let code = WIN32_ERROR(error.code().0 as u32 & 0xffff);
        return Err(if code == ERROR_CANCELLED {
            Failure::Cancelled
        } else {
            native_code("Start Windows authorization", code)
        });
    }
    if info.hProcess.is_invalid() {
        return Err(Failure::UnknownOutcome);
    }
    // SAFETY: SEE_MASK_NOCLOSEPROCESS transfers this new process handle to the caller.
    let helper = unsafe { OwnedHandle::from_raw_handle(info.hProcess.0) };
    match unsafe { WaitForSingleObject(raw(&helper), HELPER_WAIT_MS) } {
        WAIT_OBJECT_0 => {
            let mut code = 0;
            unsafe { GetExitCodeProcess(raw(&helper), &mut code) }
                .map_err(|_| Failure::UnknownOutcome)?;
            windows_protocol::decode_result(code)
        }
        _ => Err(Failure::UnknownOutcome),
    }
}
