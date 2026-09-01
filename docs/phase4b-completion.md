# Phase 4B completion evidence

Phase 4B turns the frozen v0.1 runtime into an installable, diagnosable command-line product. It does not change the frozen semantic, content, scene, choice, command, or modality contracts.

## User entry points

- `gui2tui` opens the application selector without requiring a configuration file.
- `gui2tui run --app NAME` opens a known application directly.
- `gui2tui doctor` performs bounded environment checks; `--json` emits a stable, content-free report.
- `gui2tui config init|show|check` manages the versioned XDG TOML configuration.
- `gui2tui inspect` is the low-level AT-SPI diagnostic command.
- `gui2tui endpoint` is the optional same-host modality endpoint command; its
  executable is a private libexec implementation detail.

Configuration is optional. Precedence is defaults, configuration file, then command-line overrides. Invalid or unknown configuration keys fail with a location and remediation; source values and GUI content are not echoed.

## Actual validation environment

Validation completed on 2026-08-31 using:

- macOS arm64 for the host quality suite;
- OrbStack Ubuntu 24.04 arm64 for Linux build, AT-SPI, Xvfb, GTK4, Qt6, Chrome, LibreOffice, packaging, and extracted-bundle smoke tests;
- a fresh temporary `HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, and `XDG_RUNTIME_DIR` for the release smoke test.

The final Linux bundle smoke test reported:

```text
Bootstrap: 12 nodes via AT-SPI Cache in 1.215 ms
PACKAGED_FRESH_HOME_SMOKE=PASS no_config=true action_confirmed=true password_absent=true broker_capabilities=true
```

The isolated degraded-environment suite reported:

```text
DOCTOR_REAL_SESSION_NO_ATSPI=PASS elapsed=0.004s
DEGRADED_FIRST_RUN_SELECTOR=PASS quit_responsive=true
RUNTIME_UNSAFE_REJECTED_PRIVATE_FALLBACK=PASS
DOCTOR_STALLED_ENDPOINT=PASS elapsed=1.212s
```

The stalled-endpoint check also verifies that cancellation closes the Unix socket rather than leaving a detached probe behind.

## Release artifact

The native Linux release builder produces:

```text
gui2tui-0.1.0-linux-aarch64.tar.gz
gui2tui-0.1.0-linux-aarch64.tar.gz.sha256
```

The archive contains the three binaries, dual licenses, example configuration, user documentation, dependency report, and a smoke harness that runs only the extracted binaries. It is built on Ubuntu 24.04 and requires a compatible glibc (the built artifact references symbols through GLIBC 2.39).

`deb`, `rpm`, AppImage, Flatpak, and Linux x86_64 artifacts are not built in Phase 4B.

## Diagnostics and privacy

Doctor distinguishes platform/session, configuration, private runtime directory, session D-Bus, AT-SPI, application discovery, and optional same-host endpoint state. A missing endpoint is a warning, not a semantic-runtime failure. Headless use is supported.

Product logging is opt-in, written under the private XDG runtime directory, and intentionally narrow. Doctor JSON, reports, status messages, and metrics exclude document text, text-input values, passwords, search queries, resource payloads, credential-bearing URIs, and raw accessibility bus/object identifiers.

## UX validation

Live validation covered the no-argument selector, filtering, refresh, no-application and no-AT-SPI states, contextual help for scene, reader/search/table, edit, command, and choice modes, mouse blocking behind help, GTK and Qt controls/editing/choice/password behavior, browser reader/search/table paths, same-host modality capability handling, explicit endpoint denial, application-gone handling, and content-free diagnostics.

Representative semantic TUI renderings are preserved in [GUI-to-TUI examples](gui-to-tui-examples.md). These are terminal-native semantic layouts, not framebuffer reconstructions.

## Frozen boundaries

- Core semantic/content/scene/modality IR changes in Phase 4B: **none**.
- Remote production endpoint: **not implemented**.
- New-TTY attachment to a running runtime: **not implemented**.
- Wayland static acquisition: **not implemented**.
- Electron live regression in this phase: **blocked / not tested**.
- Package-manager-native artifacts: **not implemented**.

See [Getting started](getting-started.md), [Configuration](configuration.md), [Troubleshooting](troubleshooting.md), [Deployment](deployment.md), and [Limitations](limitations.md) for user-facing guidance.
