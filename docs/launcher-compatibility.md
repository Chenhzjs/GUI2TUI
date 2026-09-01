# Saved launcher compatibility

`gui2tui app add PROGRAM` validates that `PROGRAM` is executable and stores a
direct argv, never a shell command. It does **not** claim that the application
exposes AT-SPI. A launcher becomes `verified` only after a real launch produces
an accessible application and the semantic TUI starts.

The first successful launch automatically saves a uniquely discovered AT-SPI
name. Failure classes are explicit:

- executable missing or non-executable: registration is rejected;
- strict Snap plus private D-Bus: launch is rejected immediately;
- non-zero launcher exit: reported immediately;
- no AT-SPI application before the bounded deadline: reported as an
  accessibility/session/argv failure;
- multiple new AT-SPI applications: ambiguity is rejected and `--match` is
  required;
- selector wait: redraws a countdown and Esc/q cancels without exiting the
  selector.

## Linux/Xvfb live matrix

Measured on Ubuntu 24.04 arm64 in an isolated Xvfb + D-Bus + AT-SPI session
with the v0.1.2 source build:

| Program supplied | Result | Discovered AT-SPI name | Notes |
| --- | --- | --- | --- |
| `gtk4-demo` | PASS | `gtk4-demo` | binary name only |
| `mousepad` | PASS | `mousepad` | binary name only |
| `eog` | PASS | `eog` | binary name only |
| `libreoffice` | PASS | `soffice` | mismatched name learned and persisted |
| `/usr/lib/qt6/bin/designer` | PASS | `Designer` | absolute binary path only |
| `/opt/firefox-154.0.1/firefox` | PASS | `Firefox` | binary path only |
| `google-chrome` | PASS WITH ARGV | `Google Chrome` | isolated Xvfb used a fresh profile, disabled GPU/dev-shm, and forced renderer accessibility; sandbox remained enabled |
| `pcmanfm-qt` | ACCESSIBILITY UNAVAILABLE | none | process ran but its bridge did not register in this test environment; no app-specific workaround |
| Snap `chromium` in private headless session | ENVIRONMENT BLOCKED | none | strict confinement cannot access the private session bus; rejected in under one second |

This is representative evidence, not a claim that every Linux GUI program is
compatible. GUI2TUI can only use semantics the application exposes.

## Browser example for isolated Xvfb

The fill-in wizard avoids one long shell line. Enter `google-chrome` as the
executable, then enter these arguments one per prompt:

```text
--disable-gpu
--disable-dev-shm-usage
--no-first-run
--no-default-browser-check
--disable-background-networking
--force-renderer-accessibility=complete
--user-data-dir=/path/to/a/private-test-profile
about:blank
```

For an ordinary desktop session, start with the binary name alone. Add only
arguments required by that application's documented accessibility behavior.
