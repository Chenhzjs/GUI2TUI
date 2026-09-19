use std::{
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Output},
};

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gui2tui"))
        .args(args)
        .env("HOME", root)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_RUNTIME_DIR", root)
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            "unix:path=/nonexistent/gui2tui-test-bus",
        )
        .env("DISPLAY", "credential-sentinel")
        .env("TERM", "dumb")
        .output()
        .unwrap()
}
#[test]
fn fresh_user_commands_require_no_config_or_desktop() {
    let temp = tempfile::tempdir().unwrap();
    for args in [
        vec!["--version"],
        vec!["--help"],
        vec!["config", "path"],
        vec!["config", "check"],
        vec!["config", "show"],
    ] {
        assert!(run(temp.path(), &args).status.success(), "{args:?}");
    }
    assert!(!temp.path().join("config/gui2tui/config.toml").exists());
    let help = String::from_utf8(run(temp.path(), &["--help"]).stdout).unwrap();
    for command in [
        "doctor", "config", "app", "launch", "run", "setup", "inspect", "endpoint",
    ] {
        assert!(help.contains(command));
    }
    assert!(!help.contains("--max-nodes"));
}

#[test]
fn launcher_registration_is_explicit_and_round_trips_argv() {
    let temp = tempfile::tempdir().unwrap();
    let add = run(
        temp.path(),
        &[
            "app",
            "add",
            "/usr/bin/true",
            "--id",
            "chromium",
            "--match",
            "Chromium",
            "--",
            "--force-renderer-accessibility=complete",
            "about:blank",
        ],
    );
    assert!(
        add.status.success(),
        "{}",
        String::from_utf8_lossy(&add.stderr)
    );
    assert!(
        !run(
            temp.path(),
            &["app", "add", "/usr/bin/false", "--id", "chromium"]
        )
        .status
        .success()
    );

    let listed = run(temp.path(), &["app", "list"]);
    let listing = String::from_utf8(listed.stdout).unwrap();
    assert!(
        listing
            .contains("chromium\tstatus=unverified\tprogram=/usr/bin/true\tmatch=Chromium\targs=2")
    );
    let saved = std::fs::read_to_string(temp.path().join("config/gui2tui/config.toml")).unwrap();
    assert!(saved.contains("--force-renderer-accessibility=complete"));
    assert!(saved.contains("about:blank"));

    assert!(
        run(temp.path(), &["app", "remove", "chromium"])
            .status
            .success()
    );
    assert!(
        String::from_utf8(run(temp.path(), &["app", "list"]).stdout)
            .unwrap()
            .contains("No launchers registered")
    );
}
#[test]
fn doctor_json_failure_is_bounded_and_contents_free() {
    let temp = tempfile::tempdir().unwrap();
    let started = std::time::Instant::now();
    let output = run(temp.path(), &["doctor", "--json"]);
    assert!(!output.status.success());
    assert!(started.elapsed().as_secs() < 8);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema_version"], 1);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("credential-sentinel"));
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["name"] == "session-bus" && c["level"] == "FAIL")
    );
}

#[test]
fn explicit_session_choice_controls_managed_descriptor_use() {
    let temp = tempfile::tempdir().unwrap();
    let state = temp.path().join(".local/state/gui2tui/headless");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o700)).unwrap();
    let descriptor = state.join("session.json");
    std::fs::write(&descriptor, b"not-json\n").unwrap();
    std::fs::set_permissions(&descriptor, std::fs::Permissions::from_mode(0o600)).unwrap();

    let desktop = run(temp.path(), &["--session", "desktop", "config", "path"]);
    assert!(desktop.status.success());
    assert!(!String::from_utf8_lossy(&desktop.stderr).contains("descriptor"));

    let managed = run(temp.path(), &["--session", "managed", "config", "path"]);
    assert!(!managed.status.success());
    assert!(
        String::from_utf8_lossy(&managed.stderr).contains("Managed session descriptor is invalid")
    );

    let compatible = run(temp.path(), &["doctor", "--json"]);
    let report: serde_json::Value = serde_json::from_slice(&compatible.stdout).unwrap();
    let selection = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"] == "session-selection")
        .unwrap();
    assert_eq!(selection["level"], "WARN");
    assert!(
        selection["message"]
            .as_str()
            .unwrap()
            .contains("Current desktop (compatible fallback; managed descriptor unusable)")
    );
}
#[test]
fn config_init_check_cli_override_and_invalid_file() {
    let temp = tempfile::tempdir().unwrap();
    assert!(run(temp.path(), &["config", "init"]).status.success());
    assert!(!run(temp.path(), &["config", "init"]).status.success());
    let show = run(
        temp.path(),
        &["--timeout-ms", "1200", "--no-mouse", "config", "show"],
    );
    let text = String::from_utf8(show.stdout).unwrap();
    assert!(text.contains("backend_timeout_ms = 1200") && text.contains("mouse = false"));
    std::fs::write(
        temp.path().join("config/gui2tui/config.toml"),
        "[terminal]\nmouse='secret-sentinel'",
    )
    .unwrap();
    let invalid = run(temp.path(), &["config", "check"]);
    assert!(!invalid.status.success());
    let error = String::from_utf8_lossy(&invalid.stderr);
    assert!(error.contains("line 2") && !error.contains("secret-sentinel"));
}
