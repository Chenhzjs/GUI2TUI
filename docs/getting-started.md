# Getting started

GUI2TUI v0.2 presents accessibility semantics and spatial relationships as a
responsive terminal application, with terminal controls and a document Reader.
It is useful where applications expose enough semantic information; unsupported controls
are visibly read-only, never guessed mouse clicks or anonymous actions.

## Manual installation

The v0.2.0 release candidate uses the responsive spatial presentation by
default. Build the current source on Linux with `cargo build --release` and
use `target/release/gui2tui`; `--layout flat` remains available as a
compatibility fallback.

The release exposes one user command. Keep its private implementation
components in the archive layout:

```bash
mkdir -p "$HOME/.local/bin"
install -m 755 bin/gui2tui "$HOME/.local/bin/"
mkdir -p "$HOME/.local/libexec"
cp -R libexec/gui2tui "$HOME/.local/libexec/"
```

Add that directory to PATH through your normal shell configuration. Use
`gui2tui inspect ...` for low-level diagnostics and `gui2tui endpoint ...` for
the optional same-host viewer broker; their implementation executables are
private libexec components. See `DEPENDENCIES.txt` in the archive
for actual dynamic linkage. No complete GNOME/KDE installation is required.
Official x86_64 and aarch64 archives are built natively on Ubuntu 22.04 runners.
Published archives record measured ELF requirements in
`RELEASE-MANIFEST.json`. Other architectures must build from source.

Runtime: Linux session D-Bus + AT-SPI accessibility service, a terminal with UTF-8 and
cursor/alternate-screen support, and an already running accessible GUI application.
On the tested Ubuntu environment, `dbus`, `at-spi2-core` and the GUI application's own toolkit
provide session accessibility. The binary does not link against GTK/Qt.

```bash
gui2tui doctor
gui2tui                # application selector with responsive v0.2 scenes
gui2tui --app NAME     # exact or unambiguous accessible application name
```

`--app` selects an application that is already registered in the current
AT-SPI session. It does not scan installed packages or start a binary. To save
a safe, explicit launcher and open it later:

```bash
gui2tui app add chromium       # executable; id and AT-SPI name inferred
gui2tui app list
gui2tui launch chromium
```

Or run `gui2tui app add` without an executable: the wizard asks for the
executable and optional argv one field at a time. The launcher id is inferred
from the basename and the authoritative AT-SPI name is learned on first launch.
Use advanced `--id` or `--match` only when discovery is ambiguous.

The id (`chromium`) is the user-owned launcher name. `--id` overrides it;
`--match` is the AT-SPI
application name or an unambiguous substring; use `gui2tui inspect --list`
after a manual start to discover it. Options after `--` are passed directly to
the executable. No shell is invoked. Adding an existing id is refused unless
`--replace` is explicit; remove one with `gui2tui app remove ID`.

With no arguments, the selector shows `[running] NAME` and `[launch] ID` rows.
Selecting a launcher starts it, waits up to 15 seconds for AT-SPI registration,
then opens the authoritative accessible application. Registration is still the
application/toolkit's responsibility; GUI2TUI cannot manufacture an accessibility
tree for software that exposes none.

The first successful launch compares the AT-SPI application set before and
after `exec`. If exactly one new application appears, its authoritative name is
saved automatically. `gui2tui app list` reports `status=verified` only after a
real AT-SPI launch; saving configuration alone reports `unverified`. Multiple
new applications are ambiguous and require an explicit `--match`.

Chromium may require an explicit accessibility argv:

```bash
gui2tui app add chromium --replace -- \
  --force-renderer-accessibility=complete about:blank
```

Doctor is explicit, bounded and does not read application text. No DISPLAY is not itself an
error: the terminal may be headless while a GUI session runs elsewhere on the same host.
No apps? Start an application in that desktop session, press `r`, or `d` for diagnostics.

## Managed headless session

Persistent mode creates a private Xvfb, D-Bus and AT-SPI session and verifies it
with a fresh `doctor` process:

```bash
gui2tui setup persistent
```

The session remains alive after the setup terminal closes. Future `gui2tui`,
`gui2tui inspect`, and applications started with `gui2tui launch` automatically
attach through a current-user-owned mode-0700 state directory and mode-0600
descriptor. No environment command needs to be sourced.

```bash
gui2tui setup status
gui2tui app add mousepad
gui2tui launch mousepad
gui2tui setup restart
gui2tui setup stop
```

Stopping removes the active descriptor and future invocations return to the
terminal's normal desktop session. Set `GUI2TUI_NO_MANAGED_SESSION=1` for one
explicit invocation that must ignore a running managed session.

Strict Snap confinement cannot access the helper's private session bus. A Snap
launcher is rejected before `exec` with an actionable error; use the normal
desktop session or a non-Snap package. This restriction is package/session
isolation, not a semantic-renderer limitation.

For a disposable shell whose environment is intentionally not visible to other
terminals:

```bash
gui2tui setup temporary
gui2tui setup temporary -- bash -lc 'gtk4-demo & exec gui2tui'
```

Temporary mode removes its private Xvfb/runtime directory when the child shell
or command exits. Neither mode installs packages automatically. Missing Ubuntu
dependencies are reported with the corresponding `apt install` command.

## First two minutes

1. Select an application with arrows, Enter or click. `/` starts a name filter; Enter applies,
   Esc clears. `r`/F5 refreshes discovery. The selector scrolls to keep selection visible.
2. Tab/Shift-Tab focuses controls. Enter uses a safe supported operation. Passwords remain read-only.
3. Enter a plain field to edit locally, Enter commits with GUI read-back, Esc discards. No implicit
   commit on Tab. If the GUI changes externally, cancel and reopen the field.
4. `:` opens scoped commands; F2 inside it toggles global search. Enter a document summary for Reader.
5. `?` shows help for the current view. F1 always opens help, including search/edit input. Esc returns.
6. `q` exits from Scene; Reader/Choice use Esc to return first. Ctrl-C always quits and restores terminal modes.

## Repeatable demo without a full desktop

The release ships a harmless GTK fixture and self-contained smoke harness. Testing dependencies
on Ubuntu: `xvfb dbus-x11 at-spi2-core python3-gi gir1.2-gtk-4.0 python3-pexpect python3-pyte`.
These are **test/demo dependencies**, not all required for ordinary GUI2TUI use.

```bash
bash smoke/run.sh
```

It creates a private session and fresh HOME/XDG directories, starts Xvfb and the bundled fixture,
opens the real TUI, activates a button, independently checks GUI state and exits. It retains its
exact result directory for review. Use `DISPLAY_NUMBER=:146 bash smoke/run.sh` if :145 is occupied.
It never uses Cargo or repository files. For interactive experimentation in your desktop session:
`python3 smoke/release_smoke_gtk.py`, then in another terminal `bin/gui2tui --app gui2tui-release-demo`.

Uninstall by removing only the three executables you installed and the extracted bundle.
Config removal is optional; no background daemon is installed. Do not delete live owned artifact
directories manually; leases and TTL cleanup protect active sessions.
