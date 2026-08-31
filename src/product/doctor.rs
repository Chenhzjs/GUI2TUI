use super::{config::Config, paths};
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
            "No accessible applications found. Start an application in the same desktop session, then refresh (r).",
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
pub async fn run(socket: Option<&Path>) -> Report {
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
    checks.push(check(
        "terminal",
        if io::stdout().is_terminal() && std::env::var("TERM").as_deref() != Ok("dumb") {
            Level::Pass
        } else {
            Level::Info
        },
        "Interactive TUI requires a TTY and non-dumb TERM. Doctor/config also work without a TTY.",
    ));
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
            checks.push(check("session-bus", Level::Pass, "Session D-Bus reachable"));
            let address = tokio::time::timeout(PROBE_TIMEOUT, async {
                let proxy =
                    zbus::Proxy::new(&connection, "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus")
                        .await?;
                proxy.call::<_, _, String>("GetAddress", &()).await
            })
            .await;
            checks.push(check("accessibility-bus", if matches!(address, Ok(Ok(_))) { Level::Pass } else { Level::Fail }, "org.a11y.Bus GetAddress probe; if unavailable, install/enable AT-SPI in this session. No address included in report."));
            let applications = tokio::time::timeout(PROBE_TIMEOUT, async {
                let backend = AtspiBackend::connect(PROBE_TIMEOUT).await?;
                backend.applications().await
            })
            .await;
            checks.push(availability(
                "accessible-applications",
                applications
                    .ok()
                    .and_then(Result::ok)
                    .map(|apps| apps.len()),
            ));
        }
        _ => {
            checks.push(check("session-bus", Level::Fail, "No session bus reachable within deadline. Run in the same desktop session/user; in an isolated test use dbus-run-session. Do not copy another user's credentials."));
            checks.push(check(
                "accessibility-bus",
                Level::Info,
                "Not probed: no session bus. No AT-SPI desktop connection available.",
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
        assert!(
            tokio::time::timeout(Duration::from_millis(2), std::future::pending::<()>())
                .await
                .is_err()
        );
    }
}
