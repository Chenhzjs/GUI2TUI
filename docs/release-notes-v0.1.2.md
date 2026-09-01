# GUI2TUI v0.1.2 (unreleased)

This corrective release is not published yet. It addresses failures found by
testing the v0.1.1 package in a real headless Chromium workflow.

- AT-SPI application names are learned from the uniquely new accessible
  application and persisted after a successful launch.
- Launchers have explicit `unverified`/`verified` status; saving configuration
  is no longer presented as runtime proof.
- Registration rejects missing/non-executable programs.
- Launch requests generically enable the current session's accessibility
  status before `exec`.
- Snap applications in a private D-Bus session fail immediately with package
  isolation guidance instead of waiting 15 seconds.
- Selector launch waits redraw a countdown and can be cancelled with Esc/q.
- Direct launch prints its bounded wait and classifies process exit, ambiguity,
  missing accessibility, and session mismatch.
- Doctor reports saved, verified, unverified, and current-session-incompatible
  launcher counts without collecting GUI content.
- Replaced the separate headless helper UX with `gui2tui setup persistent` and
  `gui2tui setup temporary`. Persistent sessions are automatically reused by
  later terminals for the same user.
- Unified user entry points: `gui2tui inspect` and `gui2tui endpoint`; helper
  executables are packaged as private libexec components.

v0.1.1 is marked pre-release. v0.1.2 will not be published until its Linux
real-application matrix and dual-architecture quality pipeline pass.
