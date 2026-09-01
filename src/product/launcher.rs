//! Explicit, user-owned GUI launchers. Programs and arguments are always
//! passed directly to `exec`; shell command strings are intentionally absent.

use std::{process::Stdio, time::Duration};

use crate::backend::AtspiBackend;

use super::config::LauncherConfig;

pub async fn ensure_running(
    launcher_id: &str,
    launcher: &LauncherConfig,
    backend_timeout: Duration,
) -> Result<String, String> {
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
        return Ok(name);
    }

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
            return Ok(name);
        }
        if tokio::time::Instant::now() >= deadline {
            if let Some(mut child) = child {
                tokio::spawn(async move {
                    let _ = child.wait().await;
                });
            }
            let exit_note = exited
                .map(|status| format!(" (launcher process exited with {status})"))
                .unwrap_or_default();
            return Err(format!(
                "Started launcher '{launcher_id}'{exit_note}, but no AT-SPI application matching '{}' appeared within {} ms. Ensure the program exposes accessibility in this same session; Chromium commonly needs --force-renderer-accessibility=complete",
                launcher.match_name, launcher.wait_ms,
            ));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
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
            find_match(["Chromium", "Chromium helper"].into_iter(), "chromium").unwrap(),
            Some("Chromium".into())
        );
        assert!(find_match(["App one", "App two"].into_iter(), "app").is_err());
        assert_eq!(find_match(["Firefox"].into_iter(), "chrome").unwrap(), None);
    }
}
