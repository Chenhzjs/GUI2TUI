# Getting started

GUI2TUI presents accessibility semantics as terminal controls and a document Reader.
It is useful where applications expose enough semantic information; unsupported controls
are visibly read-only, never guessed mouse clicks or anonymous actions.

## Manual installation

Use the v0.1.0 Linux archive matching `uname -m` (`x86_64` or `aarch64`).
Download it from the [GitHub Release](https://github.com/Chenhzjs/GUI2TUI/releases/tag/v0.1.0),
verify `SHA256SUMS`, extract the tarball and run `bin/gui2tui --version`. No
installer script is downloaded or executed.

Optionally copy **all three** executables into an existing directory on your PATH:

```bash
mkdir -p "$HOME/.local/bin"
install -m 755 bin/gui2tui bin/gui2tui-inspect bin/gui2tui-local "$HOME/.local/bin/"
```

Add that directory to PATH through your normal shell configuration. The inspector is also
used by host artifact TTL reaping; keep it alongside gui2tui. See `DEPENDENCIES.txt` in the archive
for actual dynamic linkage. No complete GNOME/KDE installation is required.
Official x86_64 and aarch64 archives are built natively on Ubuntu 22.04 runners.
Their measured ELF requirements are recorded in `RELEASE-MANIFEST.json`; v0.1.0
requires no GLIBC symbol newer than 2.34. Other architectures must build from source.

Runtime: Linux session D-Bus + AT-SPI accessibility service, a terminal with UTF-8 and
cursor/alternate-screen support, and an already running accessible GUI application.
On the tested Ubuntu environment, `dbus`, `at-spi2-core` and the GUI application's own toolkit
provide session accessibility. The binary does not link against GTK/Qt.

```bash
gui2tui doctor
gui2tui                # application selector, no required config
gui2tui --app NAME     # exact or unambiguous accessible application name
```

Doctor is explicit, bounded and does not read application text. No DISPLAY is not itself an
error: the terminal may be headless while a GUI session runs elsewhere on the same host.
No apps? Start an application in that desktop session, press `r`, or `d` for diagnostics.

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
