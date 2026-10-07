# GUI2TUI terminal task tests

These development tasks operate the real Rust GUI2TUI executable through a
PTY. Python sends terminal input to GUI2TUI; it never invokes GUI Action,
EditableText, focus, keyboard injection, or mouse injection APIs. Public
Accessibility reads are test assertions, not a substitute execution path.

The previous direct-pyatspi scenario runner did not validate GUI2TUI. Its
delivery results must not be cited as successful GUI2TUI tasks.

Build the Linux executable and dependency image, then run:

```sh
docker build --platform linux/amd64 -f tests/research/benchmark/Dockerfile.tasks -t gui2tui-task-tests:local .
python3 tests/research/benchmark/run_scenarios.py --image gui2tui-task-tests:local --binary target/autonomous-linux/debug/gui2tui --output /tmp/gui2tui-tasks-new
python3 -m unittest discover -s tests/research/benchmark -p 'test_*.py'
```

Each task starts a fresh container with networking disabled. The default
layout is flat; use `--layout spatial` to test spatial presentation separately.
The runner records image identity, binary hash, a fixed copy of the scenario
script, terminal transcripts, public before/after observations, and results.
Firefox navigates to a local data URL. Mousepad editing uses GUI2TUI's
configured external editor on its exported temporary representation, then
GUI2TUI commits the edit; the test never changes an application backing file.

`passed` requires a task assertion. `dispatched_requires_task_assertion` only
means the driver sent terminal input; it does not establish backend acceptance
or completion. Failed control lookup can reflect the driver, presentation, or
missing provider capability and is not automatically a backend failure.
Timeout and startup failures are recorded separately. Some tasks still lack
sufficient completion assertions; they remain unqualified.

The manifest contains five tasks for each of four applications. It is a
development task suite, not a frozen or held-out benchmark.
