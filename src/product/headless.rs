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

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum SessionChoice {
    /// Use only the desktop/session environment inherited by this process.
    Desktop,
    /// Use the existing GUI2TUI-managed headless session descriptor.
    Managed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionTopology {
    CurrentDesktop,
    ManagedHeadless,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionSelectionSource {
    Explicit,
    EnvironmentOptOut,
    NoDescriptor,
    ManagedDescriptor,
    InvalidDescriptorFallback,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSelection {
    pub topology: SessionTopology,
    pub source: SessionSelectionSource,
    pub diagnostic: Option<String>,
}

impl SessionSelection {
    pub fn label(&self) -> &'static str {
        match self.topology {
            SessionTopology::CurrentDesktop => "Current desktop",
            SessionTopology::ManagedHeadless => "Managed headless",
        }
    }

    pub fn summary(&self) -> String {
        let source = match self.source {
            SessionSelectionSource::Explicit => match self.topology {
                SessionTopology::CurrentDesktop => "explicit --session desktop",
                SessionTopology::ManagedHeadless => "explicit --session managed",
            },
            SessionSelectionSource::EnvironmentOptOut => {
                "compatible default; managed attachment disabled by environment"
            }
            SessionSelectionSource::NoDescriptor => "compatible default; no managed descriptor",
            SessionSelectionSource::ManagedDescriptor => {
                "compatible default; existing managed descriptor"
            }
            SessionSelectionSource::InvalidDescriptorFallback => {
                "compatible fallback; managed descriptor unusable"
            }
        };
        format!("{} ({source})", self.label())
    }
}

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
        .map_err(|_| "Cannot inspect managed session storage".to_owned())?;
    let correct_kind = if directory {
        metadata.is_dir()
    } else {
        metadata.is_file() && metadata.nlink() == 1
    };
    if !correct_kind
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
    {
        return Err(
            "Managed session storage must be current-user owned, private, and not a symlink"
                .to_owned(),
        );
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

fn apply_desktop() {
    // Keep explicit/default selection stable for private libexec children. This
    // is a process-local environment change and never modifies the caller's
    // shell or persistent configuration.
    unsafe {
        std::env::set_var("GUI2TUI_NO_MANAGED_SESSION", "1");
        std::env::remove_var("GUI2TUI_MANAGED_SESSION");
    }
}

fn apply_managed(session: ManagedSession) {
    unsafe {
        std::env::remove_var("GUI2TUI_NO_MANAGED_SESSION");
        std::env::set_var("DISPLAY", session.display);
        std::env::set_var("DBUS_SESSION_BUS_ADDRESS", session.session_bus_address);
        std::env::set_var("XDG_SESSION_TYPE", "x11");
        std::env::set_var("NO_AT_BRIDGE", "0");
        std::env::set_var("QT_LINUX_ACCESSIBILITY_ALWAYS_ON", "1");
        std::env::set_var("GUI2TUI_MANAGED_SESSION", "1");
    }
}

/// Select and apply one connection environment to this process and its future
/// child applications. This chooses transport only; it does not enumerate or
/// authorize an application.
///
/// # Safety invariant
/// Call this before constructing a Tokio runtime or starting any threads.
pub fn select_at_process_start(choice: Option<SessionChoice>) -> Result<SessionSelection, String> {
    match choice {
        Some(SessionChoice::Desktop) => {
            apply_desktop();
            Ok(SessionSelection {
                topology: SessionTopology::CurrentDesktop,
                source: SessionSelectionSource::Explicit,
                diagnostic: None,
            })
        }
        Some(SessionChoice::Managed) => {
            let session = load()?.ok_or_else(|| {
                "Managed session descriptor does not exist; run `gui2tui setup persistent`"
                    .to_owned()
            })?;
            apply_managed(session);
            Ok(SessionSelection {
                topology: SessionTopology::ManagedHeadless,
                source: SessionSelectionSource::Explicit,
                diagnostic: None,
            })
        }
        None if std::env::var_os("GUI2TUI_NO_MANAGED_SESSION").is_some() => {
            apply_desktop();
            Ok(SessionSelection {
                topology: SessionTopology::CurrentDesktop,
                source: SessionSelectionSource::EnvironmentOptOut,
                diagnostic: None,
            })
        }
        None => match load() {
            Ok(Some(session)) => {
                apply_managed(session);
                Ok(SessionSelection {
                    topology: SessionTopology::ManagedHeadless,
                    source: SessionSelectionSource::ManagedDescriptor,
                    diagnostic: None,
                })
            }
            Ok(None) => {
                apply_desktop();
                Ok(SessionSelection {
                    topology: SessionTopology::CurrentDesktop,
                    source: SessionSelectionSource::NoDescriptor,
                    diagnostic: None,
                })
            }
            Err(error) => {
                apply_desktop();
                Ok(SessionSelection {
                    topology: SessionTopology::CurrentDesktop,
                    source: SessionSelectionSource::InvalidDescriptorFallback,
                    diagnostic: Some(format!(
                        "Managed descriptor was not used: {error}. Run `gui2tui setup status` or select `--session managed` for a blocking check."
                    )),
                })
            }
        },
    }
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
