use std::{
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
    for command in ["doctor", "config", "run"] {
        assert!(help.contains(command));
    }
    assert!(!help.contains("--max-nodes"));
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
