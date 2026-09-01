# Getting started

GUI2TUI presents accessibility semantics as terminal controls and a document Reader.
It is useful where applications expose enough semantic information; unsupported controls
are visibly read-only, never guessed mouse clicks or anonymous actions.

## Manual installation

Use the v0.1.1 Linux archive matching `uname -m` (`x86_64` or `aarch64`).
Download it from the [GitHub Release](https://github.com/Chenhzjs/GUI2TUI/releases/tag/v0.1.1),
verify `SHA256SUMS`, extract the tarball and run `bin/gui2tui --version`. No
installer script is downloaded or executed.

Optionally copy the runtime executables into an existing directory on your PATH.
If the extracted package contains `gui2tui-headless`, install it too:

```bash
mkdir -p "$HOME/.local/bin"
install -m 755 bin/gui2tui bin/gui2tui-inspect bin/gui2tui-local "$HOME/.local/bin/"
test ! -e bin/gui2tui-headless || \
    install -m 755 bin/gui2tui-headless "$HOME/.local/bin/"
```

Add that directory to PATH through your normal shell configuration. The inspector is also
used by host artifact TTL reaping; keep it alongside gui2tui. See `DEPENDENCIES.txt` in the archive
for actual dynamic linkage. No complete GNOME/KDE installation is required.
Official x86_64 and aarch64 archives are built natively on Ubuntu 22.04 runners.
Their measured ELF requirements are recorded in `RELEASE-MANIFEST.json`; v0.1.1
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

`--app` selects an application that is already registered in the current
AT-SPI session. It does not scan installed packages or start a binary. To save
a safe, explicit launcher and open it later:

```bash
gui2tui app add chromium       # executable; id and AT-SPI name inferred
gui2tui app list
gui2tui launch chromium
```

Or run `gui2tui app add` without an executable to fill in `Executable`, launcher
name, expected AT-SPI name, and optional argv one field at a time. This wizard
requires an interactive terminal and shows defaults that Enter accepts.

The id (`chromium`) is the user-owned launcher name. `--id` overrides it;
`--match` is the AT-SPI
application name or an unambiguous substring; use `gui2tui-inspect --list`
after a manual start to discover it. Options after `--` are passed directly to
the executable. No shell is invoked. Adding an existing id is refused unless
`--replace` is explicit; remove one with `gui2tui app remove ID`.

With no arguments, the selector shows `[running] NAME` and `[launch] ID` rows.
Selecting a launcher starts it, waits up to 15 seconds for AT-SPI registration,
then opens the authoritative accessible application. Registration is still the
application/toolkit's responsibility; GUI2TUI cannot manufacture an accessibility
tree for software that exposes none.

Chromium may require an explicit accessibility argv:

```bash
gui2tui app add chromium --replace -- \
  --force-renderer-accessibility=complete about:blank
```

Doctor is explicit, bounded and does not read application text. No DISPLAY is not itself an
error: the terminal may be headless while a GUI session runs elsewhere on the same host.
No apps? Start an application in that desktop session, press `r`, or `d` for diagnostics.

## One-command headless session

The package includes a `gui2tui-headless` helper. It creates a private Xvfb,
D-Bus and AT-SPI session, runs `gui2tui doctor`, then opens an interactive shell:

```bash
bin/gui2tui-headless
```

Inside that shell, start a GUI application and GUI2TUI normally. Both processes
inherit the same private accessibility session:

```bash
gtk4-demo &
gui2tui
```

Launchers saved inside that helper's private HOME are intentionally temporary.
For persistent launchers, provide your normal configuration HOME deliberately,
or register them in the ordinary same-user desktop/SSH session. A program that
is already running outside the helper's private D-Bus/AT-SPI session is not
visible inside it.

Or run everything as one command:

```bash
bin/gui2tui-headless -- bash -lc './my-gui-app & exec bin/gui2tui'
```

Use `--doctor-only` for a non-interactive environment check. A valid headless
session may still report WARN for “no accessible applications” before an app is
started and for the optional same-host viewer; WARN is not a blocking doctor
failure. Missing Ubuntu dependencies are reported with the corresponding
`apt install` command. The helper never installs packages automatically, does
not modify the parent shell, and removes its private Xvfb/runtime directory when
the child shell or command exits.

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
