# GUI2TUI v0.1.1

GUI2TUI v0.1.1 is a focused usability update for headless Linux hosts. Core
semantic, content, scene, modality and runtime contracts are unchanged.

## What changed

- Added packaged `bin/gui2tui-headless` helper.
- The helper creates a private Xvfb, D-Bus and AT-SPI session, enables
  accessibility, runs `gui2tui doctor`, then opens a shell or runs a command.
- Xvfb display allocation and readiness are capability-based rather than a
  fixed sleep; the private display and runtime directory are cleaned on exit.
- Missing Ubuntu dependencies produce an explicit `apt install` suggestion;
  the helper never installs packages automatically.
- README now includes real Chrome/Firefox, LibreOffice Writer, GTK and Qt
  GUI-to-TUI examples.
- Added explicit saved application launchers: `gui2tui app add/list/remove`,
  `gui2tui launch ID`, and `[launch]` rows alongside `[running]` applications
  in the selector. Launchers use direct argv execution, bounded AT-SPI waiting,
  atomic private configuration writes, and never invoke a shell.
- `gui2tui app add PROGRAM` infers ordinary defaults; bare `gui2tui app add`
  provides an interactive fill-in wizard for executable, name, AT-SPI match and
  optional arguments.
- Release validation accepts immutable v0.1.0 packages while checking the
  helper whenever it is present in newer packages.

## Headless quick start

```bash
tar -xzf gui2tui-0.1.1-linux-x86_64.tar.gz
cd gui2tui-0.1.1-linux-x86_64
bin/gui2tui-headless
```

Inside the new shell, start the GUI application and GUI2TUI in the same private
session:

```bash
chromium --force-renderer-accessibility=complete about:blank &
gui2tui
```

In an isolated root container Chromium may additionally require `--no-sandbox`;
do not use that option as ordinary-user guidance.

`gui2tui-inspect --list` enumerates applications already registered with the
current AT-SPI session. It does not scan installed executables or launch them.
Likewise, `gui2tui --app NAME` selects an already running accessible application.
Use `gui2tui app add` plus `gui2tui launch ID` when GUI2TUI should start it.

## Language and text boundary

UTF-8 application names, labels and document content—including Chinese—can be
displayed when exposed by AT-SPI and supported by the terminal font. Local
single-line editing is Unicode-scalar-safe and has Chinese-character unit
coverage. GUI2TUI does not implement an IME protocol or grapheme-cluster-aware
editing, so complex input methods and combined emoji sequences remain limited.

## Compatibility and safety

All v0.1.0 safety rules remain in force: password content is never read or
written, anonymous actions are refused, stale identities are rejected, partial
documents do not claim complete coverage, and the GUI remains the authoritative
state after operations.

See [Getting started](getting-started.md), [Compatibility](compatibility.md), and
[Limitations](limitations.md).
