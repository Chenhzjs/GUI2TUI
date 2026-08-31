# Troubleshooting

Start with `gui2tui doctor`. PASS/WARN/FAIL/INFO indicate capability, not an exception traceback.
Exit 0 means no blocking FAIL, including valid headless/no-viewer or no-app WARN states;
exit 1 means at least one FAIL. `--verbose` adds timeout information, not arbitrary D-Bus error bodies.
`--json` is schema-versioned machine-readable output.

| Situation | What to do |
| --- | --- |
| Session bus unavailable | Use the desktop's user/session. SSH alone does not create a desktop. For an isolated test use dbus-run-session + Xvfb/fixtures. Do not copy another user's credentials. |
| org.a11y.Bus unavailable | Check AT-SPI service installation/activation in that session. Doctor explicitly calls GetAddress and reports failure; no speculative flag names. |
| No applications | Start an accessibility-enabled GUI program there; press r/F5 in selector. d opens diagnostics. |
| DISPLAY unset | Not automatically a failure. Headless terminal operation needs access to the GUI's accessibility session, not a viewer. |
| Application exited | Old controls become non-interactive. F5 searches for a fresh generation; b opens selector, d diagnoses. q quits. |
| Backend disappeared | The existing bounded recovery retries; if still unavailable F5 retries explicitly. No old identities are revived. |
| Text unavailable/read-only | The application's accessibility interface may be missing or unreliable. Do not retry writes by guessed keys. Runtime quarantine resets on a fresh application generation. |
| No local viewer | Valid headless mode. References remain inspectable; an available artifact can be materialized by explicit m. No fake Open and no unsolicited transfer. |
| Viewer denies/fails | Check local authorization/handler and retry explicitly after reconnect. Failed old grants/operations cannot complete later. |
| Invalid config | Run config check; fix reported path/line/schema. Delete only your config file if you intentionally want defaults. |
| Unsafe runtime directory | Supply an owned, nonsymlink 0700 XDG_RUNTIME_DIR, or unset it to use the verified private fallback. |
| Too-small terminal | Resize, use arrows/PageUp/PageDown or help scrolling. UTF-8 and a non-dumb TERM are required. |

## Safe support report

```bash
gui2tui doctor --json --report ./gui2tui-report.json
```

The new file is mode 0600 and is never overwritten. Report excludes GUI/document/input/password
text, search queries, app names, payloads, resource/credential URIs, environment addresses and
arbitrary recent logs. Only version/platform, presence flags, bounded probe results/counts and
config validity are gathered. This standalone command does not attach another running TUI's
metrics or recent errors. F12 lets you inspect current-session metrics locally; review anything
before sharing. Config parse errors do not quote the offending value.

Doctor uses a 1200 ms deadline per remote probe (at most four sequential remote probes, including
an explicitly requested endpoint). No apps are traversed. Normal GUI2TUI startup does **not** run doctor.

Ctrl-C and controlled panic restore terminal modes. SIGKILL cannot restore them from inside the
dead process; use the shell's `reset` if needed. Same-PTY SIGUSR1 detach/SIGUSR2 resume is supported,
but a new terminal connecting to an old process is NOT IMPLEMENTED.
