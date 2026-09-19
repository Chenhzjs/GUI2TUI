use super::{
    config::Config,
    headless::{SessionSelection, SessionTopology},
    launcher, paths,
};
use crate::backend::AtspiBackend;
use serde::Serialize;
use std::{
    io::{self, IsTerminal, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    time::{Duration, Instant},
};

const PROBE_TIMEOUT: Duration = Duration::from_millis(1200);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Level {
    Pass,
    Warn,
    Fail,
    Info,
}
#[derive(Clone, Debug, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub level: Level,
    pub message: String,
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub version: &'static str,
    pub os: &'static str,
    pub arch: &'static str,
    pub elapsed_ms: u128,
    pub checks: Vec<Check>,
    pub exclusions: &'static str,
}
impl Report {
    pub fn healthy(&self) -> bool {
        !self.checks.iter().any(|c| c.level == Level::Fail)
    }
    pub fn text(&self, verbose: bool) -> String {
        let mut text = format!(
            "GUI2TUI {} diagnostics ({}/{})\n",
            self.version, self.os, self.arch
        );
        for check in &self.checks {
            text.push_str(&format!(
                "{} {}: {}\n",
                format!("{:?}", check.level).to_uppercase(),
                check.name,
                check.message
            ));
        }
        if verbose {
            text.push_str(&format!("Bounded probe deadline: {} ms each; total {} ms. No raw D-Bus errors or environment addresses are collected.\n", PROBE_TIMEOUT.as_millis(), self.elapsed_ms));
        }
        text
    }
    pub fn write_private(&self, path: &Path) -> io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        serde_json::to_writer_pretty(&mut file, self)?;
        file.write_all(b"\n")?;
        file.sync_all()
    }
}
fn check(name: &'static str, level: Level, message: impl Into<String>) -> Check {
    Check {
        name,
        level,
        message: message.into(),
    }
}
fn availability(name: &'static str, result: Option<usize>) -> Check {
    match result {
        Some(0) => check(
            name,
            Level::Warn,
            "No accessible applications found. Start one in this same session, or save a launcher with `gui2tui app add`, then refresh (r). Installed binaries are not AT-SPI registrations.",
        ),
        Some(count) => check(
            name,
            Level::Pass,
            format!("{count} accessible application(s); no names or GUI contents collected"),
        ),
        None => check(
            name,
            Level::Fail,
            "Desktop accessibility service unavailable. Use the same user's desktop session; check the session bus and AT-SPI packages, then retry.",
        ),
    }
}

fn installation_checks(selection: &SessionSelection) -> Vec<Check> {
    let mut checks = Vec::new();
    checks.push(check(
        "installation-entry",
        if paths::current_executable_is_usable().is_ok() {
            Level::Pass
        } else {
            Level::Fail
        },
        "The running GUI2TUI entry must be a readable executable file; no source checkout or Cargo runtime is required.",
    ));

    let inspector = paths::companion("gui2tui-inspect");
    checks.push(check(
        "helper-inspector",
        if inspector.is_ok() {
            Level::Pass
        } else {
            Level::Fail
        },
        if inspector.is_ok() {
            "Required private inspector is executable through the current installation layout."
        } else {
            "Required private inspector is missing or not executable. Reinstall GUI2TUI into one prefix; do not copy only bin/gui2tui."
        },
    ));

    let headless = paths::companion("headless-session");
    checks.push(check(
        "helper-managed-headless",
        if headless.is_ok() {
            Level::Pass
        } else if selection.topology == SessionTopology::ManagedHeadless {
            Level::Fail
        } else {
            Level::Warn
        },
        if headless.is_ok() {
            "Managed Headless setup helper is executable. Its system dependencies are checked without installation when setup is requested."
        } else {
            "Managed Headless setup/recovery is unavailable because its helper is missing or not executable. Desktop operation can continue; reinstall before using `gui2tui setup`."
        },
    ));

    let local = paths::companion("gui2tui-local");
    checks.push(check(
        "helper-same-host-modality",
        if local.is_ok() {
            Level::Pass
        } else {
            Level::Warn
        },
        if local.is_ok() {
            "Optional same-host modality helper is executable. It is not a semantic backend or Remote Companion."
        } else {
            "Optional same-host modality helper is unavailable. Core semantic TUI operation is unaffected; reinstall only if that optional endpoint is needed."
        },
    ));

    match paths::user_install_manifest() {
        Ok(Some(_)) => {
            let uninstaller = paths::companion("uninstall-user");
            checks.push(check(
                "user-prefix-install",
                if uninstaller.is_ok() {
                    Level::Pass
                } else {
                    Level::Fail
                },
                if uninstaller.is_ok() {
                    "Auditable user-prefix installation and exact-file uninstaller are present. User configuration and runtime data are outside the uninstall manifest."
                } else {
                    "The user-prefix installation marker exists, but its exact-file uninstaller is missing or not executable. Reinstall before removal."
                },
            ));
        }
        Ok(None) => checks.push(check(
            "user-prefix-install",
            Level::Info,
            "No managed user-prefix install marker found. This is normal for a source/developer or manually extracted run; use the bundled installer for auditable uninstall.",
        )),
        Err(_) => checks.push(check(
            "user-prefix-install",
            Level::Fail,
            "The user-prefix install marker is unsafe or inaccessible. Do not delete the prefix recursively; reinstall or inspect only the GUI2TUI libexec files.",
        )),
    }
    checks
}

fn configured_locale_is_utf8() -> Option<bool> {
    ["LC_ALL", "LC_CTYPE", "LANG"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
        .map(|value| {
            let normalized = value.to_ascii_lowercase().replace('-', "");
            normalized.contains("utf8")
        })
}

fn terminal_checks() -> Vec<Check> {
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
    let mut checks = vec![check(
        "terminal-interactive",
        if interactive {
            Level::Pass
        } else {
            Level::Warn
        },
        if interactive {
            "stdin and stdout are attached to a TTY/PTY."
        } else {
            "Doctor can run without a TTY, but the interactive TUI requires both stdin and stdout attached to one TTY/PTY."
        },
    )];
    let term_usable = std::env::var("TERM").is_ok_and(|term| !term.is_empty() && term != "dumb");
    checks.push(check(
        "terminal-type",
        if term_usable {
            Level::Pass
        } else if interactive {
            Level::Fail
        } else {
            Level::Info
        },
        if term_usable {
            "TERM is set and not `dumb`. Terminal-emulator-specific enhanced-key behavior is not qualified by this probe."
        } else {
            "TERM is absent or `dumb`. Non-interactive Doctor remains available, but do not start the TUI until a real terminal supplies a usable TERM."
        },
    ));
    checks.push(match configured_locale_is_utf8() {
        Some(true) => check(
            "terminal-utf8",
            Level::Pass,
            "The active locale environment declares UTF-8.",
        ),
        Some(false) if interactive => check(
            "terminal-utf8",
            Level::Fail,
            "The active locale environment does not declare UTF-8. Select an installed UTF-8 locale before starting the TUI.",
        ),
        Some(false) => check(
            "terminal-utf8",
            Level::Warn,
            "The active locale environment does not declare UTF-8. Doctor can continue, but interactive TUI use requires UTF-8.",
        ),
        None => check(
            "terminal-utf8",
            Level::Info,
            "UTF-8 locale NOT CHECKED because LC_ALL, LC_CTYPE and LANG are unset.",
        ),
    });
    if interactive {
        checks.push(match crossterm::terminal::size() {
            Ok((width, height)) if width > 0 && height > 0 => check(
                "terminal-size",
                Level::Pass,
                format!("Terminal size {width}x{height} is readable. The responsive renderer has no fixed product-wide minimum; very small terminals may collapse presentation."),
            ),
            _ => check(
                "terminal-size",
                Level::Fail,
                "Terminal dimensions are unavailable or zero; resize or use a functional PTY before starting the TUI.",
            ),
        });
    } else {
        checks.push(check(
            "terminal-size",
            Level::Info,
            "Terminal dimensions NOT CHECKED without an interactive TTY/PTY.",
        ));
    }
    checks.push(check(
        "terminal-modes",
        Level::Info,
        "Raw mode, alternate screen and enhanced keyboard reporting are NOT CHECKED because Doctor does not alter terminal state. They are acquired and restored only by the interactive TUI.",
    ));
    checks
}

async fn endpoint_probe(socket: &Path) -> io::Result<bool> {
    use crate::modality::wire::{Request, Response};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    // Same existing capability protocol, cancellable connect/read/write. A
    // dribbling or hung peer cannot leave a background diagnostic thread alive.
    let mut stream = tokio::net::UnixStream::connect(socket).await?;
    let request = serde_json::to_vec(&Request::Capabilities {})?;
    stream
        .write_all(&(request.len() as u32).to_be_bytes())
        .await?;
    stream.write_all(&request).await?;
    let length = stream.read_u32().await? as usize;
    if length > 64 * 1024 {
        return Err(io::Error::other("capability frame limit"));
    }
    let mut payload = vec![0; length];
    stream.read_exact(&mut payload).await?;
    Ok(matches!(
        serde_json::from_slice::<Response>(&payload),
        Ok(Response::Capabilities { .. })
    ))
}

/// Only explicitly invoked by the user; never part of initial semantic bootstrap.
pub async fn run(socket: Option<&Path>, selection: &SessionSelection) -> Report {
    let started = Instant::now();
    let mut checks = vec![check(
        "platform",
        if cfg!(target_os = "linux") {
            Level::Pass
        } else {
            Level::Warn
        },
        if cfg!(target_os = "linux") {
            "Linux runtime supported"
        } else {
            "Development/build platform only; live desktop operation requires Linux AT-SPI"
        },
    )];
    checks.push(check(
        "session-selection",
        if selection.diagnostic.is_some() {
            Level::Warn
        } else {
            Level::Info
        },
        match selection.diagnostic.as_deref() {
            Some(diagnostic) => format!("Selected {}. {diagnostic}", selection.summary()),
            None => format!(
                "Selected {}. Session selection chooses connection environment only; application authority still requires a fresh current AT-SPI enumeration and explicit application selection.",
                selection.summary()
            ),
        },
    ));
    checks.extend(installation_checks(selection));
    let display = std::env::var_os("DISPLAY").is_some();
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let session = match std::env::var("XDG_SESSION_TYPE").as_deref() {
        Ok("x11") => "X11",
        Ok("wayland") => "Wayland",
        _ => "unspecified",
    };
    checks.push(check("desktop", Level::Info, format!("Session={session}; DISPLAY set={display}; WAYLAND_DISPLAY set={wayland}. Missing display is valid for a headless terminal with access to a desktop's accessibility bus.")));
    checks.push(check(
        "wayland-capture",
        Level::Info,
        "Wayland static capture NOT IMPLEMENTED; semantic AT-SPI use is independent of capture.",
    ));
    if rustix::process::geteuid().as_raw() == 0 {
        checks.push(check("user", Level::Warn, "Running as root; prefer the desktop user. Root does not grant access to another user's session."));
    }
    checks.extend(terminal_checks());
    let config_ok = paths::config_path()
        .ok()
        .is_some_and(|path| Config::load(&path).is_ok());
    checks.push(check(
        "configuration",
        if config_ok { Level::Pass } else { Level::Fail },
        if config_ok {
            "Valid configuration or defaults (no file required)"
        } else {
            "Invalid/unreadable configuration. Run gui2tui config check for path and line guidance."
        },
    ));
    if let Some(config) = paths::config_path()
        .ok()
        .and_then(|path| Config::load(&path).ok())
    {
        checks.push(match config.interaction.complex_text.as_ref() {
            None => check(
                "external-text-handler",
                Level::Info,
                "No complex-text handler configured (valid). Core GUI2TUI remains available; qualified complex plain text stays read-only.",
            ),
            Some(handler) if launcher::validate_program(&handler.program).is_ok() => check(
                "external-text-handler",
                Level::Pass,
                "Configured direct-argv handler is structurally valid and executable, with exactly one standalone {file}. It was not started and no candidate was created.",
            ),
            Some(_) => check(
                "external-text-handler",
                Level::Warn,
                "Configured direct-argv handler is structurally valid but its program is unavailable or not executable. Core GUI2TUI remains available; fix that configured program before external editing.",
            ),
        });
        let verified = config
            .launchers
            .values()
            .filter(|entry| entry.verified)
            .count();
        let incompatible = config
            .launchers
            .values()
            .filter(|entry| launcher::validate_launch_environment(&entry.program).is_err())
            .count();
        let unavailable = config
            .launchers
            .values()
            .filter(|entry| launcher::validate_program(&entry.program).is_err())
            .count();
        checks.push(check(
            "saved-launchers",
            if incompatible == 0 && unavailable == 0 {
                Level::Info
            } else {
                Level::Warn
            },
            format!(
                "{} saved; {verified} verified by a prior AT-SPI launch; {} unverified; {unavailable} executable unavailable; {incompatible} incompatible with the current session/package isolation",
                config.launchers.len(),
                config.launchers.len().saturating_sub(verified),
            ),
        ));
    }
    let runtime = paths::runtime_dir();
    checks.push(check("runtime-directory", if runtime.is_ok() { Level::Pass } else { Level::Fail }, "Requires a current-user-owned 0700 directory, no symlinks. When XDG_RUNTIME_DIR is absent, a verified private temporary fallback is used."));
    let health = runtime.and_then(|path| crate::runtime::artifacts::health_counts(&path));
    checks.push(match health {
        Ok((namespaces, leased)) => check("artifact-ownership", Level::Pass, format!("Observed {namespaces} namespaces / {leased} live leases (scan bounded at 4096). No payload read or deletion; startup recovery separately validates complete ownership.")),
        Err(_) => check("artifact-ownership", Level::Warn, "Artifact namespace cannot be safely inspected (permissions, transient or foreign entry). No deletion performed; inspect private runtime directory."),
    });
    let dbus = tokio::time::timeout(PROBE_TIMEOUT, zbus::Connection::session()).await;
    match dbus {
        Ok(Ok(connection)) => {
            checks.push(check(
                "session-bus",
                Level::Pass,
                format!("Session D-Bus reachable for {}", selection.label()),
            ));
            let address = tokio::time::timeout(PROBE_TIMEOUT, async {
                let proxy =
                    zbus::Proxy::new(&connection, "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus")
                        .await?;
                proxy.call::<_, _, String>("GetAddress", &()).await
            })
            .await;
            if matches!(address, Ok(Ok(_))) {
                checks.push(check(
                    "accessibility-bus",
                    Level::Pass,
                    "org.a11y.Bus returned an Accessibility bus address. The address is not included in this report.",
                ));
                let registry = tokio::time::timeout(PROBE_TIMEOUT, async {
                    let backend = AtspiBackend::connect(PROBE_TIMEOUT).await?;
                    let applications = backend.applications().await?;
                    Ok::<_, crate::backend::BackendError>(applications.len())
                })
                .await;
                match registry {
                    Ok(Ok(count)) => {
                        checks.push(check(
                            "accessibility-registry",
                            Level::Pass,
                            "AT-SPI registry is reachable through the selected session.",
                        ));
                        checks.push(availability("accessible-applications", Some(count)));
                        checks.push(check(
                            "application-semantics",
                            Level::Info,
                            if count == 0 {
                                "NOT CHECKED: no current application is registered. This is distinct from a session or AT-SPI connection failure."
                            } else {
                                "NOT CHECKED by Doctor: application presence proves registry visibility only. Missing controls or operations are an application Accessibility limitation, not evidence that the installation or session is broken."
                            },
                        ));
                    }
                    _ => {
                        checks.push(check(
                            "accessibility-registry",
                            Level::Fail,
                            "Accessibility bus address exists, but the AT-SPI registry or enumeration is unavailable within the bounded probe. Check AT-SPI services in this selected session.",
                        ));
                        checks.push(check(
                            "accessible-applications",
                            Level::Info,
                            "Not probed: AT-SPI registry connection failed; this is not an application-not-found result.",
                        ));
                        checks.push(check(
                            "application-semantics",
                            Level::Info,
                            "NOT CHECKED: application semantics require a reachable registry and explicit current application selection.",
                        ));
                    }
                }
            } else {
                checks.push(check(
                    "accessibility-bus",
                    Level::Fail,
                    "Session D-Bus is reachable, but org.a11y.Bus did not return an Accessibility bus address within the bounded probe. Enable/install AT-SPI in this selected session; no address or raw D-Bus error is reported.",
                ));
                checks.push(check(
                    "accessibility-registry",
                    Level::Info,
                    "Not probed: the selected session did not provide an Accessibility bus address.",
                ));
                checks.push(check(
                    "accessible-applications",
                    Level::Info,
                    "Not probed: Accessibility infrastructure is unavailable; this is not a zero-application result.",
                ));
                checks.push(check(
                    "application-semantics",
                    Level::Info,
                    "NOT CHECKED: application semantics cannot be assessed without AT-SPI connectivity.",
                ));
            }
        }
        _ => {
            checks.push(check("session-bus", Level::Fail, format!("No session bus reachable for {} within deadline. Select `--session desktop` for the inherited desktop environment or start/check the managed session before selecting `--session managed`. Do not copy another user's credentials.", selection.label())));
            checks.push(check(
                "accessibility-bus",
                Level::Info,
                "Not probed: no session bus. No AT-SPI desktop connection available.",
            ));
            checks.push(check(
                "accessibility-registry",
                Level::Info,
                "Not probed: no session bus.",
            ));
            checks.push(check(
                "accessible-applications",
                Level::Info,
                "Not probed: no session bus; this is not a zero-application result.",
            ));
            checks.push(check(
                "application-semantics",
                Level::Info,
                "NOT CHECKED: application semantics require a connected selected session and fresh application choice.",
            ));
        }
    }
    if let Some(socket) = socket {
        let connected = matches!(
            tokio::time::timeout(PROBE_TIMEOUT, endpoint_probe(socket)).await,
            Ok(Ok(true))
        );
        checks.push(check("same-host-endpoint", if connected { Level::Pass } else { Level::Warn }, if connected { "Local broker capabilities received; no authorization or payload sent" } else { "Local broker unavailable. Semantic TUI and explicit host materialization remain usable. Start/configure gui2tui-local only if a viewer is wanted." }));
    } else {
        checks.push(check("same-host-endpoint", Level::Warn, "No viewer endpoint configured (valid headless mode). References remain inspectable; available artifacts can be explicitly materialized on this host."));
    }
    Report {
        schema_version: 1,
        version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        elapsed_ms: started.elapsed().as_millis(),
        checks,
        exclusions: "No GUI text, input values, passwords, queries, payloads, resource URIs, environment addresses, app names, or arbitrary logs. Running-session metrics/recent errors are not attached by this standalone command; inspect F12 locally.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn doctor_distinguishes_no_apps_from_no_backend() {
        assert_eq!(availability("apps", Some(0)).level, Level::Warn);
        assert_eq!(availability("apps", None).level, Level::Fail);
        assert_eq!(availability("apps", Some(2)).level, Level::Pass);
    }
    #[test]
    fn report_is_structured_private_and_never_overwrites() {
        let report = Report {
            schema_version: 1,
            version: "0.1.0",
            os: "linux",
            arch: "aarch64",
            elapsed_ms: 1,
            checks: vec![availability("apps", Some(2))],
            exclusions: "contents excluded",
        };
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["checks"][0]["level"], "PASS");
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("report.json");
        report.write_private(&path).unwrap();
        assert!(report.write_private(&path).is_err());
    }
    #[tokio::test]
    async fn hung_probe_deadline_is_bounded() {
        use tokio::io::AsyncReadExt;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("endpoint");
        let listener = match tokio::net::UnixListener::bind(&path) {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                // Some sandboxed macOS runners deny AF_UNIX creation even in a
                // private temporary directory. The live Linux harness covers
                // this timeout path with a real socket.
                return;
            }
            Err(error) => panic!("cannot create timeout test socket: {error}"),
        };
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let length = socket.read_u32().await.unwrap() as usize;
            let mut request = vec![0; length];
            socket.read_exact(&mut request).await.unwrap();
            assert!(matches!(
                serde_json::from_slice::<crate::modality::wire::Request>(&request),
                Ok(crate::modality::wire::Request::Capabilities {})
            ));
            // Deliberately never send a response. Cancellation must close the
            // client socket, rather than leaving a detached blocking reader.
            assert_eq!(socket.read(&mut [0]).await.unwrap(), 0);
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(50), endpoint_probe(&path))
                .await
                .is_err()
        );
        tokio::time::timeout(Duration::from_secs(1), server)
            .await
            .unwrap()
            .unwrap();
    }
}
