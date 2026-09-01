# Configuration (schema version 1)

No file is required. Missing file = safe defaults. No startup write or migration occurs.

```bash
gui2tui config path
gui2tui config show
gui2tui config check
gui2tui config init       # explicit, create-new only; never overwrites
```

Path: `$XDG_CONFIG_HOME/gui2tui/config.toml` when absolute, otherwise
`$HOME/.config/gui2tui/config.toml`. This same portable XDG fallback is used on the macOS
development host. Relative XDG_CONFIG_HOME is ignored per the XDG convention. Config files
are UTF-8 TOML with a 64 KiB read limit. Read-only config works. Init uses mode 0600.

```toml
version = 1

[runtime]
backend_timeout_ms = 5000
event_queue_capacity = 2048

[terminal]
mouse = true

[launchers.chromium]
program = "chromium"
args = ["--force-renderer-accessibility=complete", "about:blank"]
match_name = "Chromium"
wait_ms = 15000
verified = false # managed by GUI2TUI; true only after a real AT-SPI launch
```

`backend_timeout_ms`: 50–30000 inclusive, existing per-backend operation deadline.
`event_queue_capacity`: 4–65536 inclusive, bounded event buffer. Small queues may deliberately
cause overflow/full resync; 2048 remains the tested default. `mouse` enables terminal mouse capture.

Precedence: built-in defaults < config file < explicit CLI.
`--timeout-ms`, `--event-buffer-capacity` are retained developer overrides but hidden from brief help;
`--no-mouse` is user-visible. Example: `gui2tui --no-mouse config show` prints effective configuration.
Unknown keys/types, unsupported versions and out-of-range values are rejected, never silently clamped.
Syntax/type errors report file and line without echoing source values. A bad file must be fixed;
CLI flags cannot make an invalid file valid. Launchers are user-owned process
configuration, not toolkit-specific semantic behavior. Prefer managing them
without hand-editing TOML:

```bash
gui2tui app add chromium
gui2tui app add       # interactive fill-in wizard
gui2tui app list
gui2tui app remove chromium
```

Launcher ids are 1–64 ASCII letters/digits plus `.`, `_`, or `-`. A launcher
contains one executable, at most 128 argv entries, an expected AT-SPI name, and
a bounded 100–120000 ms registration wait. It never contains a shell command,
environment override, password, privilege escalation, or automatic sandbox
disable. Add advanced argv after `--`; override inferred values with `--id` and
`--match`. Existing ids require explicit `--replace`. The config is atomically
written mode 0600 and a config-file symlink is refused.

Do not hand-edit `verified`: it records observed runtime evidence, not user
intent. A uniquely discovered AT-SPI name replaces the initial basename match
after the first successful launch.

Only existing runtime knobs are exposed. Content budgets, artifact size/TTL bounds and authorization
continue using their tested policies; they are **not** pretend TOML settings. Local broker handlers
remain explicit user-owned CLI configuration, not remote executable instructions:

```bash
gui2tui-local serve --socket "$XDG_RUNTIME_DIR/viewer.sock" \
  --mime image/png --handler-program /usr/bin/eog --authorization once
gui2tui --modality-socket "$XDG_RUNTIME_DIR/viewer.sock"
```

Only use an installed viewer you trust. Omitting `--authorization` prompts locally; do not use
`--recording-handler` for real viewing (it is a test handler). Persistent trust UI NOT IMPLEMENTED.

## Logs and paths

Normal startup is quiet. `--log-level info|debug` opts into `product.log` in the private runtime directory.
It logs product lifecycle metadata only, not raw AT-SPI/debug payloads. RUST_LOG cannot expand this
user binary's filter into content-bearing third-party logs. This deliberately narrow log is bounded by
the small number of lifecycle messages, truncated on the next explicit logged run. F12 shows richer
contents-free **current-session** metrics. Inspector debug output is for deliberate local diagnosis,
not automatically attached to support reports.

Runtime artifacts and the log prefer `$XDG_RUNTIME_DIR/gui2tui`; XDG_RUNTIME_DIR must be absolute,
owned by the current user, a real directory and private (0700). If absent, a verified
`$TMPDIR/gui2tui-runtime-UID` (system temp when TMPDIR absent) is used. Unsafe explicitly configured
runtime paths fail with guidance. Owned artifact leases/markers and recovery remain unchanged;
the exact legacy temp namespace is also safely checked on upgrade. No generic /tmp glob cleanup.

`XDG_CACHE_HOME` is not currently used: Semantic/Content caches are session-memory only.
No document bodies are persisted there. Broker sockets use the explicit `--socket` path; prefer
your private runtime directory. No automatic broker launch or endpoint probe occurs on first frame.
