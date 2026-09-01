//! Explicit, user-owned GUI launchers. Programs and arguments are always
//! passed directly to `exec`; shell command strings are intentionally absent.

use std::{process::Stdio, time::Duration};

use crate::backend::AtspiBackend;

use super::config::LauncherConfig;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchOutcome {
    pub application_name: String,
    /// The configured match did not identify the application, but exactly one
    /// new AT-SPI application appeared after exec. Callers may persist this
    /// authoritative name for subsequent launches.
    pub discovered_name: bool,
}

pub fn validate_program(program: &str) -> Result<std::path::PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;

    let resolved = if program.contains('/') {
        Some(std::path::PathBuf::from(program))
    } else {
        std::env::var_os("PATH").and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|path| path.join(program))
                .find(|candidate| candidate.is_file())
        })
    }
    .ok_or_else(|| format!("Executable '{program}' was not found in PATH"))?;
    let metadata = resolved.metadata().map_err(|error| {
        format!(
            "Cannot inspect executable '{}': {error}",
            resolved.display()
        )
    })?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return Err(format!(
            "Program '{}' is not an executable file",
            resolved.display()
        ));
    }
    Ok(resolved)
}

pub async fn ensure_running(
    launcher_id: &str,
    launcher: &LauncherConfig,
    backend_timeout: Duration,
) -> Result<LaunchOutcome, String> {
    let backend = AtspiBackend::connect(backend_timeout)
        .await
        .map_err(|_| "Desktop accessibility service unavailable; run gui2tui doctor".to_owned())?;
    if let Some(name) = find_match(
        backend
            .applications()
            .await
            .map_err(|error| error.to_string())?
            .iter()
            .map(|application| application.name.as_str()),
        &launcher.match_name,
    )? {
        return Ok(LaunchOutcome {
            application_name: name,
            discovered_name: false,
        });
    }

    validate_program(&launcher.program)?;
    validate_launch_environment(&launcher.program)?;
    // This is the generic desktop accessibility opt-in. Applications remain
    // responsible for registering with AT-SPI; failure is non-fatal because
    // some sessions expose read-only status properties while toolkit bridges
    // may already be active.
    let _ = tokio::time::timeout(backend_timeout, request_session_accessibility()).await;

    let before = backend
        .applications()
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|application| application.name)
        .collect::<std::collections::BTreeSet<_>>();

    let mut command = tokio::process::Command::new(&launcher.program);
    command
        .args(&launcher.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(false);
    let child = command.spawn().map_err(|error| {
        format!(
            "Cannot start launcher '{launcher_id}' program '{}': {error}",
            launcher.program
        )
    })?;

    let mut child = Some(child);
    let mut exited = None;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(launcher.wait_ms);
    loop {
        if let Some(process) = child.as_mut()
            && let Some(status) = process
                .try_wait()
                .map_err(|error| format!("Cannot monitor launcher '{launcher_id}': {error}"))?
        {
            // Some desktop launchers intentionally fork/activate another
            // process and exit. Continue the bounded AT-SPI wait.
            if !status.success() {
                return Err(format!(
                    "Launcher '{launcher_id}' exited with {status} before exposing an AT-SPI application"
                ));
            }
            exited = Some(status);
            child = None;
        }
        let applications = backend
            .applications()
            .await
            .map_err(|error| error.to_string())?;
        if let Some(name) = find_match(
            applications
                .iter()
                .map(|application| application.name.as_str()),
            &launcher.match_name,
        )? {
            // Reap it asynchronously when it eventually exits. Dropping a
            // Child does not terminate the graphical application.
            if let Some(mut child) = child {
                tokio::spawn(async move {
                    let _ = child.wait().await;
                });
            }
            return Ok(LaunchOutcome {
                application_name: name,
                discovered_name: false,
            });
        }
        let newly_visible = applications
            .iter()
            .map(|application| application.name.as_str())
            .filter(|name| !before.iter().any(|existing| existing == *name))
            .collect::<Vec<_>>();
        if newly_visible.len() == 1 {
            if let Some(mut child) = child {
                tokio::spawn(async move {
                    let _ = child.wait().await;
                });
            }
            return Ok(LaunchOutcome {
                application_name: newly_visible[0].to_owned(),
                discovered_name: true,
            });
        }
        if newly_visible.len() > 1 {
            return Err(format!(
                "Launcher '{launcher_id}' exposed multiple new AT-SPI applications: {}. Re-register it with --match NAME",
                newly_visible
                    .iter()
                    .map(|name| format!("'{name}'"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if tokio::time::Instant::now() >= deadline {
            if let Some(mut child) = child {
                tokio::spawn(async move {
                    let _ = child.wait().await;
                });
            }
            let exit_note = exited
                .map(|status| format!(" (launcher process exited with {status})"))
                .unwrap_or_else(|| " (launcher process is still running)".into());
            return Err(format!(
                "Started launcher '{launcher_id}'{exit_note}, but no accessible application appeared within {} ms (configured match '{}'). The program may not expose AT-SPI, may require accessibility-specific argv, or may be attached to a different desktop session. Start it manually and run `gui2tui-inspect --list`; then re-register with the required argv or --match name.",
                launcher.wait_ms, launcher.match_name,
            ));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

pub fn validate_launch_environment(program: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        use std::path::PathBuf;

        let resolved = validate_program(program).ok().or_else(|| {
            if program.contains('/') {
                Some(PathBuf::from(program))
            } else {
                None
            }
        });
        let bus_address = std::env::var("DBUS_SESSION_BUS_ADDRESS").ok();
        if is_snap_launcher_in_private_bus(resolved.as_deref(), bus_address.as_deref()) {
            return Err(format!(
                "Cannot launch Snap program '{program}' inside this private D-Bus session: strict Snap confinement cannot reach the session bus. Use the normal desktop session, or register a non-Snap build of the application."
            ));
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = program;
    Ok(())
}

#[cfg(target_os = "linux")]
fn is_snap_launcher_in_private_bus(
    program: Option<&std::path::Path>,
    bus_address: Option<&str>,
) -> bool {
    program.is_some_and(|path| path.starts_with("/snap/bin"))
        && bus_address.is_some_and(|address| !address.contains("/run/user/"))
}

#[cfg(target_os = "linux")]
async fn request_session_accessibility() -> Result<(), String> {
    let connection = zbus::Connection::session()
        .await
        .map_err(|error| error.to_string())?;
    let proxy = zbus::Proxy::new(
        &connection,
        "org.a11y.Bus",
        "/org/a11y/bus",
        "org.a11y.Status",
    )
    .await
    .map_err(|error| error.to_string())?;
    proxy
        .set_property("IsEnabled", true)
        .await
        .map_err(|error| error.to_string())?;
    proxy
        .set_property("ScreenReaderEnabled", true)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
async fn request_session_accessibility() -> Result<(), String> {
    Ok(())
}

fn find_match<'a>(
    names: impl Iterator<Item = &'a str>,
    selector: &str,
) -> Result<Option<String>, String> {
    let names: Vec<_> = names.collect();
    if let Some(exact) = names
        .iter()
        .find(|name| name.eq_ignore_ascii_case(selector))
    {
        return Ok(Some((*exact).to_owned()));
    }
    let selector = selector.to_lowercase();
    let matches: Vec<_> = names
        .iter()
        .filter(|name| name.to_lowercase().contains(&selector))
        .collect();
    match matches.as_slice() {
        [] => Ok(None),
        [name] => Ok(Some((***name).to_owned())),
        _ => Err(format!(
            "AT-SPI match '{}' is ambiguous: {}",
            selector,
            matches
                .iter()
                .map(|name| format!("'{name}'"))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_is_exact_first_and_ambiguity_safe() {
        assert_eq!(
            find_match(
                ["Example Editor", "Example Editor helper"].into_iter(),
                "example editor"
            )
            .unwrap(),
            Some("Example Editor".into())
        );
        assert!(find_match(["App one", "App two"].into_iter(), "app").is_err());
        assert_eq!(
            find_match(["Another Application"].into_iter(), "missing").unwrap(),
            None
        );
    }

    #[test]
    fn registration_rejects_missing_program_and_accepts_executable_path() {
        assert!(validate_program("gui2tui-program-that-cannot-exist-19f37").is_err());
        assert!(validate_program(std::env::current_exe().unwrap().to_str().unwrap()).is_ok());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn snap_private_session_is_rejected_without_app_specific_logic() {
        use std::path::Path;

        assert!(is_snap_launcher_in_private_bus(
            Some(Path::new("/snap/bin/example")),
            Some("unix:path=/tmp/dbus-private")
        ));
        assert!(!is_snap_launcher_in_private_bus(
            Some(Path::new("/snap/bin/example")),
            Some("unix:path=/run/user/1000/bus")
        ));
        assert!(!is_snap_launcher_in_private_bus(
            Some(Path::new("/usr/bin/example")),
            Some("unix:path=/tmp/dbus-private")
        ));
    }
}
