# Getting started

GUI2TUI v0.3 presents accessibility semantics, spatial relationships, and
verified operations as a responsive terminal application. It provides native
controls, bounded Value adjustment, a document Reader, and optional configured
interaction for qualified complete plain text. Insufficient capability remains
visibly read-only; operations are never guessed.

## User-prefix installation

Build the current source on Linux with the pinned dependencies, then install
the complete runtime layout without root:

```bash
cargo build --release --locked --bins
./scripts/install-user.sh --prefix "$HOME/.local"
export PATH="$HOME/.local/bin:$PATH"
```

An extracted bundle that contains the installer uses the same contract:

```bash
./install-user.sh --prefix "$HOME/.local"
```

The default prefix is `$HOME/.local`; `--prefix` also accepts another absolute,
current-user-owned path, including paths containing spaces. Never run the
installer with `sudo`. It refuses symlink install directories and every
pre-existing target rather than overwriting unrelated files. Reinstall by
using the recorded uninstaller first or by choosing another prefix.

The installed layout is:

```text
PREFIX/bin/gui2tui
PREFIX/libexec/gui2tui/gui2tui-inspect
PREFIX/libexec/gui2tui/gui2tui-local
PREFIX/libexec/gui2tui/headless-session
PREFIX/libexec/gui2tui/uninstall-user
PREFIX/libexec/gui2tui/install-manifest-v1
```

Only `gui2tui` belongs on `PATH`. `gui2tui inspect ...`, `gui2tui endpoint ...`
and `gui2tui setup ...` find their private components relative to the running
main executable, so they do not depend on the checkout, Cargo target directory
or current working directory. `gui2tui-local` is optional for core semantic
operation but is installed so the explicit same-host modality endpoint remains
available. No separate runtime data bundle is required. Existing archive
`DEPENDENCIES.txt` files describe their own binary linkage; current-source
package/ABI qualification remains a later v0.7 phase.

Runtime: Linux session D-Bus + AT-SPI accessibility service, a terminal with UTF-8 and
cursor/alternate-screen support, and an already running accessible GUI application.
On the tested Ubuntu environment, `dbus`, `at-spi2-core` and the GUI application's own toolkit
provide session accessibility. The binary does not link against GTK/Qt.

```bash
gui2tui --session desktop doctor
gui2tui --session desktop                # current desktop application selector
gui2tui --session desktop --app NAME     # exact or unambiguous current application
```

`--session desktop` uses only the desktop/session environment inherited by the
current process. `--session managed` requires an existing valid managed
headless descriptor created by `gui2tui setup persistent`. Session choice only
selects the connection environment; it does not select or authorize an
application.

For compatibility, omitting `--session` still reuses a valid managed
descriptor when one exists and otherwise uses the inherited desktop
environment. Startup and Doctor identify the resulting topology. An invalid
or stopped descriptor produces a warning and uses the inherited environment
only in this unspecified compatibility mode. Explicit `--session managed`
fails instead of falling back; explicit `--session desktop` never reads the
descriptor. `GUI2TUI_NO_MANAGED_SESSION=1` remains a compatible opt-out, but
new commands should prefer the explicit flag.

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

Doctor is explicit, bounded and does not read application text. It checks the
actual running entry, private helpers, optional same-host helper, user-install
marker, configuration, handler executability, terminal basics, selected
session D-Bus, Accessibility bus, registry and application count. No DISPLAY
is not itself an error: the terminal may be headless while a GUI session runs
elsewhere on the same host. No apps? Start an application in that exact
session, press `r`, or `d` for diagnostics. Application presence does not prove
that every control exposes sufficient semantics; Doctor labels that
application-level assessment NOT CHECKED instead of declaring the deployment
broken.

## Managed headless session

Persistent mode creates a private Xvfb, D-Bus and AT-SPI session and verifies it
with a fresh `doctor` process:

```bash
gui2tui setup persistent
```

The session remains alive after the setup terminal closes. Future `gui2tui`,
`gui2tui inspect`, and applications started with `gui2tui launch` can select it
through a current-user-owned mode-0700 state directory and mode-0600
descriptor. No environment command needs to be sourced:

```bash
gui2tui setup status
gui2tui --session managed doctor
gui2tui --session managed app add mousepad
gui2tui --session managed launch mousepad
gui2tui setup restart
gui2tui setup stop
```

Stopping removes the active descriptor. A later explicit managed selection
then reports that the descriptor is absent; it never creates a session or
falls back. Use `--session desktop` for an invocation that must ignore a
running managed session.

Descriptor loading checks schema, bounded fields, current-user ownership,
private permissions and the recorded supervisor. Connection probing then
distinguishes an unavailable selected bus from a healthy registry containing
zero applications. GUI2TUI does not delete an invalid descriptor during these
checks; `setup status`, `restart`, and `stop` remain the explicit recovery
commands.

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

### Validation-only Docker and SSH topology

The repository contains a reproducible v0.7C live environment for the exact
qualified Ubuntu 24.04 arm64 container topology:

```bash
RESULT_DIR=/tmp/gui2tui-v07c-evidence \
  tests/live/v07c_headless_qualification.sh
```

It builds the current source, installs it into the container user's prefix,
starts Managed Xvfb and controlled GTK fixtures, drives a real `docker exec
-it` TTY and a loopback-only OpenSSH PTY, then stops/uninstalls GUI2TUI and
removes its exact temporary container, image, key and port. It requires Docker,
OpenSSH client tools and Python 3 on the host. It does not use a host GUI,
personal SSH keys, privileged mode, host PID namespace or a complete desktop.

This is live qualification infrastructure, not a supported production image
or general Docker installer. Its result applies only to the recorded topology;
package/architecture integration remains 0.7E work. A Docker or SSH PTY result
also does not qualify a Linux virtual-console TTY.

The repository also contains a bounded 0.7D Headless Wayland/XWayland live
environment:

```bash
RESULT_DIR=/tmp/gui2tui-v07d-evidence \
  tests/live/v07d_wayland_qualification.sh
```

It starts Weston's real headless backend with software rendering, a private
session D-Bus/AT-SPI registry, controlled native GTK/Qt clients and a separately
identified XWayland GTK client. It requires Docker and Python 3 on the host,
but no host GUI socket, graphical device, privileged container or complete
desktop. The recorded Ubuntu 24.04 arm64 topology is qualified with explicit
limitations; this validation fixture is not a production image or an implicit
claim for other compositors, architectures, desktop sessions or Wayland over
SSH. Wayland static capture remains unavailable.

## First two minutes

1. Select an application with arrows, Enter or click. `/` starts a name filter; Enter applies,
   Esc clears. `r`/F5 refreshes discovery. The selector scrolls to keep selection visible.
2. Tab/Shift-Tab focuses controls. Enter uses a safe supported operation. Passwords remain read-only.
3. Enter a plain field to edit locally, Enter commits with GUI read-back, Esc discards. No implicit
   commit on Tab. If the GUI changes externally, cancel and reopen the field.
4. `:` opens scoped commands; F2 inside it toggles global search. Enter a document summary for Reader.
5. `?` shows help for the current view. F1 always opens help, including search/edit input. Esc returns.
6. `q` exits from Scene; Reader/Choice use Esc to return first. Ctrl-C always quits and restores terminal modes.

## Optional complex-text handler

Ordinary startup and native single-line/Value interaction require no external
editor. To enable external interaction for targets GUI2TUI independently
qualifies as complete, bounded, non-secret multiline plain text, configure one
local executable and argv template:

```toml
[interaction.complex_text]
program = "custom-editor-command"
args = ["--wait", "{file}"]
```

The command is executed directly without a shell, and exactly one standalone
`{file}` is required. It names a private GUI2TUI-owned candidate—not an
application backing file. After the handler exits, GUI2TUI checks target
identity and concurrent GUI changes, writes only through public AT-SPI, and
independently reads the entire GUI text back. Missing configuration leaves the
target readable and reports that external editing is not configured. See
[Configuration](configuration.md) for the complete contract.

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

## Safe uninstall

Stop an owned Managed Headless session first, then run the installed exact-file
uninstaller:

```bash
gui2tui setup stop                  # only when a Managed descriptor exists
"$HOME/.local/libexec/gui2tui/uninstall-user" --prefix "$HOME/.local"
```

The uninstaller verifies its private manifest and every remaining installed
file before removing only the five recorded GUI2TUI executables/scripts plus
the manifest. It refuses changed files, symlinks, unsafe ownership/permissions
or an existing Managed descriptor. It never recursively removes the prefix and
never kills processes by name. Unrelated prefix files and
`$XDG_CONFIG_HOME/gui2tui/config.toml` are retained, as are user-selected
private recovery candidates and runtime state. Do not delete live artifact or
Managed state directories manually; use existing lifecycle commands and TTL
cleanup.
