//! The elevated helper's transport: a named pipe owned by the app, and a UAC launch of
//! `smd.exe --helper <pipe> <app pid>`.
//!
//! Security model:
//! - The app creates the pipe with `FILE_FLAG_FIRST_PIPE_INSTANCE` and a random name, so no
//!   other process can own that name first. Remote clients are rejected.
//! - The helper connects with `SECURITY_IDENTIFICATION`, so the pipe owner can never impersonate
//!   its elevated token, then checks the pipe server's process id and program path match the app.
//! - The helper only runs the typed operations in [`crate::helper::Op`]. Each one re-checks its
//!   input against the built-in catalogs (cleanup rules, tweaks, startup items).

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, ERROR_PIPE_CONNECTED, HANDLE};
use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeServerProcessId, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::core::{HSTRING, PCWSTR};

use crate::helper::Op;

/// How long the app waits for the helper to connect after the UAC prompt.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(120);
const STILL_ACTIVE: u32 = 259;

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub op: Op,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    #[serde(default)]
    pub value: serde_json::Value,
    #[serde(default)]
    pub error: Option<String>,
}

/// The app's side of a helper session.
pub struct Client {
    writer: File,
    reader: BufReader<File>,
    process: HANDLE,
    next_id: u64,
}

// The process handle is only used from the thread that owns the client (behind a mutex).
unsafe impl Send for Client {}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.send(&Op::Exit);
        let _ = unsafe { CloseHandle(self.process) };
    }
}

fn random_suffix() -> String {
    let mut bytes = [0u8; 16];
    // A fresh BLAKE3 hash of time, pid and an address is plenty for an unguessable name.
    let seed = format!(
        "{:?}{}{:p}",
        std::time::SystemTime::now(),
        std::process::id(),
        &bytes
    );
    bytes.copy_from_slice(&blake3::hash(seed.as_bytes()).as_bytes()[..16]);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn create_pipe(name: &str) -> io::Result<File> {
    let wide = HSTRING::from(name);
    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(wide.as_ptr()),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            64 * 1024,
            64 * 1024,
            0,
            None,
        )
    };
    if handle.is_invalid() {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_handle(handle.0) })
}

fn launch_elevated(exe: &Path, args: &str) -> io::Result<HANDLE> {
    let file = HSTRING::from(exe.as_os_str());
    let params = HSTRING::from(args);
    // Debug builds only: SMD_HELPER_NO_UAC=1 starts the helper without admin rights, to test the
    // pipe and the checks end to end without a UAC prompt.
    let no_uac = cfg!(debug_assertions) && std::env::var_os("SMD_HELPER_NO_UAC").is_some();
    let verb = HSTRING::from(if no_uac { "open" } else { "runas" });
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    match unsafe { ShellExecuteExW(&mut info) } {
        Ok(()) if !info.hProcess.is_invalid() => Ok(info.hProcess),
        Ok(()) => Err(io::Error::other("the helper did not start")),
        Err(e) if e.code() == ERROR_CANCELLED.to_hresult() => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "admin rights were not granted",
        )),
        Err(e) => Err(io::Error::other(e)),
    }
}

fn exited(process: HANDLE) -> bool {
    let mut code = 0u32;
    unsafe { GetExitCodeProcess(process, &mut code) }.is_ok() && code != STILL_ACTIVE
}

impl Client {
    /// Shows the UAC prompt and waits for the helper to connect.
    pub fn start(exe: &Path) -> io::Result<Client> {
        let name = format!(
            r"\\.\pipe\smd-helper-{}-{}",
            std::process::id(),
            random_suffix()
        );
        let server = create_pipe(&name)?;
        let process = launch_elevated(exe, &format!("--helper {name} {}", std::process::id()))?;

        let raw = server.as_raw_handle() as usize;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let r = unsafe { ConnectNamedPipe(HANDLE(raw as *mut _), None) };
            let ok = match r {
                Ok(()) => true,
                Err(e) => e.code() == ERROR_PIPE_CONNECTED.to_hresult(),
            };
            let _ = tx.send(ok);
        });
        let started = Instant::now();
        loop {
            match rx.recv_timeout(Duration::from_millis(250)) {
                Ok(true) => break,
                Ok(false) => {
                    let _ = unsafe { CloseHandle(process) };
                    return Err(io::Error::other("the helper could not connect"));
                }
                Err(_) => {
                    if exited(process) || started.elapsed() > CONNECT_TIMEOUT {
                        // Unblock the waiting thread by connecting ourselves, then give up.
                        let _ = std::fs::OpenOptions::new()
                            .read(true)
                            .write(true)
                            .open(&name);
                        let _ = unsafe { CloseHandle(process) };
                        return Err(io::Error::other("the helper stopped before it connected"));
                    }
                }
            }
        }
        let reader = BufReader::new(server.try_clone()?);
        Ok(Client {
            writer: server,
            reader,
            process,
            next_id: 1,
        })
    }

    pub fn is_alive(&self) -> bool {
        !exited(self.process)
    }

    fn send(&mut self, op: &Op) -> io::Result<u64> {
        let id = self.next_id;
        self.next_id += 1;
        let mut line = serde_json::to_vec(&Request { id, op: op.clone() })?;
        line.push(b'\n');
        self.writer.write_all(&line)?;
        self.writer.flush()?;
        Ok(id)
    }

    /// Sends one operation and waits for its answer.
    pub fn call<T: DeserializeOwned>(&mut self, op: &Op) -> io::Result<T> {
        let id = self.send(op)?;
        let mut line = String::new();
        if self.reader.read_line(&mut line)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "the admin helper closed",
            ));
        }
        let resp: Response = serde_json::from_str(&line)?;
        if resp.id != id {
            return Err(io::Error::other("the admin helper answered out of order"));
        }
        if let Some(e) = resp.error {
            return Err(io::Error::other(e));
        }
        serde_json::from_value(resp.value).map_err(io::Error::other)
    }
}

fn process_path(pid: u32) -> io::Result<String> {
    let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
        .map_err(io::Error::other)?;
    let mut buf = vec![0u16; 1024];
    let mut len = buf.len() as u32;
    let r = unsafe {
        QueryFullProcessImageNameW(
            h,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
    };
    let _ = unsafe { CloseHandle(h) };
    r.map_err(io::Error::other)?;
    Ok(String::from_utf16_lossy(&buf[..len as usize]))
}

/// The helper's side: connects to the app's pipe and checks it is really the app.
pub fn connect_to_app(pipe: &str, app_pid: u32) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    if !pipe.starts_with(r"\\.\pipe\smd-helper-") || pipe.contains(['/', '"']) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "bad pipe name"));
    }
    // SECURITY_IDENTIFICATION: the pipe owner may identify us but never act as us.
    const SECURITY_IDENTIFICATION: u32 = 1 << 16;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .security_qos_flags(SECURITY_IDENTIFICATION)
        .open(pipe)?;
    let mut server_pid = 0u32;
    unsafe { GetNamedPipeServerProcessId(HANDLE(file.as_raw_handle()), &mut server_pid) }
        .map_err(io::Error::other)?;
    if server_pid != app_pid {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "the pipe belongs to another process",
        ));
    }
    let me = std::env::current_exe()?;
    let server = process_path(server_pid)?;
    if !crate::helper::same_file(Path::new(&server), &me) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "the pipe belongs to another program",
        ));
    }
    Ok(file)
}
