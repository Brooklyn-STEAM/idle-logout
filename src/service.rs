// Runs as a SYSTEM service (`idle-logout.exe --service`) and starts the app in every signed-in
// session that doesn't have it, so a user who kills the app gets it back within seconds.
use std::collections::HashSet;
use std::ffi::OsString;
use std::sync::mpsc;
use std::time::Duration;
use windows_service::service::{ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::{define_windows_service, service_dispatcher};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
use windows_sys::Win32::System::RemoteDesktop::{
    WTS_CURRENT_SERVER_HANDLE, WTS_PROCESS_INFOW, WTS_SESSION_INFOW, WTSEnumerateProcessesW, WTSEnumerateSessionsW,
    WTSFreeMemory, WTSQueryUserToken,
};
use windows_sys::Win32::System::Threading::{CREATE_UNICODE_ENVIRONMENT, CreateProcessAsUserW, PROCESS_INFORMATION, STARTUPINFOW};

const NAME: &str = "IdleLogout";
const POLL: Duration = Duration::from_secs(3);

define_windows_service!(ffi_main, service_main);

pub fn run() {
    if let Err(e) = service_dispatcher::start(NAME, ffi_main) {
        eprintln!("not started by the service manager: {e}");
    }
}

fn service_main(_: Vec<OsString>) {
    let (stop_tx, stop) = mpsc::channel();
    let handler = move |control| match control {
        ServiceControl::Stop => {
            stop_tx.send(()).ok();
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    };
    let Ok(status) = service_control_handler::register(NAME, handler) else { return };
    let set = |state, accept| {
        status.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: accept,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::ZERO,
            process_id: None,
        })
    };
    set(ServiceState::Running, ServiceControlAccept::STOP).ok();
    let Ok(exe) = std::env::current_exe() else { return };
    while stop.recv_timeout(POLL) == Err(mpsc::RecvTimeoutError::Timeout) {
        let running = sessions_running_app();
        for session in user_sessions() {
            if !running.contains(&session) {
                launch(session, &exe);
            }
        }
    }
    set(ServiceState::Stopped, ServiceControlAccept::empty()).ok();
}

fn user_sessions() -> Vec<u32> {
    let (mut ptr, mut count) = (std::ptr::null_mut::<WTS_SESSION_INFOW>(), 0);
    if unsafe { WTSEnumerateSessionsW(WTS_CURRENT_SERVER_HANDLE, 0, 1, &mut ptr, &mut count) } == 0 || ptr.is_null() {
        return Vec::new();
    }
    let ids: Vec<u32> = unsafe { std::slice::from_raw_parts(ptr, count as usize) }.iter().map(|s| s.SessionId).collect();
    unsafe { WTSFreeMemory(ptr.cast()) };
    // Session 0 is services only; a session with no user token has nobody signed in.
    ids.into_iter().filter(|&id| id != 0 && user_token(id).map(close).is_some()).collect()
}

fn sessions_running_app() -> HashSet<u32> {
    let (mut ptr, mut count) = (std::ptr::null_mut::<WTS_PROCESS_INFOW>(), 0);
    if unsafe { WTSEnumerateProcessesW(WTS_CURRENT_SERVER_HANDLE, 0, 1, &mut ptr, &mut count) } == 0 || ptr.is_null() {
        return HashSet::new();
    }
    // Names point into the same buffer, so read them before it's freed.
    let found = unsafe { std::slice::from_raw_parts(ptr, count as usize) }
        .iter()
        .filter(|p| !p.pProcessName.is_null())
        .filter(|p| unsafe { wide_to_string(p.pProcessName) }.eq_ignore_ascii_case("idle-logout.exe"))
        .map(|p| p.SessionId)
        .collect();
    unsafe { WTSFreeMemory(ptr.cast()) };
    found
}

unsafe fn wide_to_string(p: *const u16) -> String {
    let len = (0..).take_while(|&i| unsafe { *p.add(i) } != 0).count();
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, len) })
}

fn user_token(session: u32) -> Option<HANDLE> {
    let mut token = std::ptr::null_mut();
    (unsafe { WTSQueryUserToken(session, &mut token) } != 0).then_some(token)
}

fn close(handle: HANDLE) {
    unsafe { CloseHandle(handle) };
}

fn launch(session: u32, exe: &std::path::Path) {
    let Some(token) = user_token(session) else { return };
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let mut command = wide(&format!("\"{}\"", exe.display()));
    let desktop = wide(r"winsta0\default");
    let mut env = std::ptr::null_mut();
    let startup = STARTUPINFOW { cb: size_of::<STARTUPINFOW>() as u32, lpDesktop: desktop.as_ptr() as _, ..Default::default() };
    let mut info = PROCESS_INFORMATION::default();
    unsafe {
        // The user's own environment, so %APPDATA% and friends point at their profile, not SYSTEM's.
        CreateEnvironmentBlock(&mut env, token, 0);
        let ok = CreateProcessAsUserW(
            token,
            std::ptr::null(),
            command.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            CREATE_UNICODE_ENVIRONMENT,
            env,
            std::ptr::null(),
            &startup,
            &mut info,
        );
        if ok != 0 {
            close(info.hProcess);
            close(info.hThread);
        }
        if !env.is_null() {
            DestroyEnvironmentBlock(env);
        }
    }
    close(token);
}
