//! Discovery of an explicitly configured, user-owned managed headless session.
//!
//! The process environment is changed only during single-threaded binary
//! startup, before Tokio or any D-Bus client is constructed.

use serde::{Deserialize, Serialize};
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSession {
    pub schema_version: u32,
    pub supervisor_pid: u32,
    pub display: String,
    pub session_bus_address: String,
}

pub fn state_root_from(
    xdg_state: Option<PathBuf>,
    home: Option<PathBuf>,
) -> Result<PathBuf, String> {
    let base = match xdg_state.filter(|path| path.is_absolute()) {
        Some(path) => path,
        None => home
            .filter(|path| path.is_absolute())
            .ok_or_else(|| "Set HOME or an absolute XDG_STATE_HOME".to_owned())?
            .join(".local/state"),
    };
    Ok(base.join("gui2tui/headless"))
}

pub fn state_root() -> Result<PathBuf, String> {
    state_root_from(
        std::env::var_os("XDG_STATE_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    )
}

pub fn descriptor_path() -> Result<PathBuf, String> {
    Ok(state_root()?.join("session.json"))
}

fn verify_private(path: &Path, directory: bool) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| format!("Cannot inspect managed session path {}", path.display()))?;
    let correct_kind = if directory {
        metadata.is_dir()
    } else {
        metadata.is_file() && metadata.nlink() == 1
    };
    if !correct_kind
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
    {
        return Err(format!(
            "Managed session path {} must be current-user owned, private, and not a symlink",
            path.display()
        ));
    }
    Ok(())
}

pub fn load() -> Result<Option<ManagedSession>, String> {
    let descriptor = descriptor_path()?;
    if !descriptor.exists() {
        return Ok(None);
    }
    let root = descriptor
        .parent()
        .ok_or_else(|| "Managed session descriptor has no parent".to_owned())?;
    verify_private(root, true)?;
    verify_private(&descriptor, false)?;
    let bytes =
        fs::read(&descriptor).map_err(|_| "Cannot read managed session descriptor".to_owned())?;
    if bytes.len() > 4096 {
        return Err("Managed session descriptor exceeds 4 KiB".into());
    }
    let session: ManagedSession = serde_json::from_slice(&bytes)
        .map_err(|_| "Managed session descriptor is invalid".to_owned())?;
    if session.schema_version != 1
        || session.supervisor_pid == 0
        || !session.display.starts_with(':')
        || session.display.len() > 32
        || !session.display[1..]
            .bytes()
            .all(|byte| byte.is_ascii_digit())
        || !session.session_bus_address.starts_with("unix:path=")
        || session.session_bus_address.len() > 4096
        || session.session_bus_address.contains(['\n', '\r', '\0'])
    {
        return Err("Managed session descriptor contains invalid fields".into());
    }
    #[cfg(target_os = "linux")]
    {
        let process = PathBuf::from(format!("/proc/{}", session.supervisor_pid));
        let metadata = fs::metadata(process).map_err(|_| {
            "Managed headless supervisor is not running; run `gui2tui setup persistent`".to_owned()
        })?;
        if metadata.uid() != rustix::process::geteuid().as_raw() {
            return Err("Managed headless supervisor belongs to another user".into());
        }
    }
    Ok(Some(session))
}

/// Apply the managed session to this process and future child applications.
///
/// # Safety invariant
/// Call this before constructing a Tokio runtime or starting any threads.
pub fn apply_at_process_start() -> Result<bool, String> {
    if std::env::var_os("GUI2TUI_NO_MANAGED_SESSION").is_some() {
        return Ok(false);
    }
    let Some(session) = load()? else {
        return Ok(false);
    };
    // SAFETY: both GUI2TUI entry points invoke this in their synchronous main,
    // before Tokio, tracing, D-Bus connections, or any application thread.
    unsafe {
        std::env::set_var("DISPLAY", session.display);
        std::env::set_var("DBUS_SESSION_BUS_ADDRESS", session.session_bus_address);
        std::env::set_var("XDG_SESSION_TYPE", "x11");
        std::env::set_var("NO_AT_BRIDGE", "0");
        std::env::set_var("QT_LINUX_ACCESSIBILITY_ALWAYS_ON", "1");
        std::env::set_var("GUI2TUI_MANAGED_SESSION", "1");
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_path_prefers_xdg_and_falls_back_to_home() {
        assert_eq!(
            state_root_from(Some("/state".into()), Some("/home/user".into())).unwrap(),
            PathBuf::from("/state/gui2tui/headless")
        );
        assert_eq!(
            state_root_from(None, Some("/home/user".into())).unwrap(),
            PathBuf::from("/home/user/.local/state/gui2tui/headless")
        );
        assert!(state_root_from(None, None).is_err());
    }
}
