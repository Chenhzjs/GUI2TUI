# Compatibility matrix

Validated on 2026-08-28 in Ubuntu 24.04 arm64, Xvfb/X11, AT-SPI 2.52, GTK 4.14.5, and Qt 6.4.2 through PyQt 6.6.1.

| Feature | GTK4 | Qt6 | Browser |
| --- | --- | --- | --- |
| Application discovery | PASS | PASS | PASS (Chrome 152) |
| Button role | PASS (`button`) | PASS (`button`) | PASS (`button`) |
| Button action | PASS (`Click`) | PASS (`Press`) | PARTIAL: two anonymous actions; explicit index 0 worked |
| Plain TextInput | PASS (`text`) | PASS (`text`) | PARTIAL (`entry`; value unavailable) |
| Password TextInput | PASS (`password text`) | PASS (`password text`) | PASS (`password text`) |
| Password redaction | PASS | PASS | PASS: sentinel absent from normal/verbose/TUI/log |
| Checkbox state | PASS | PASS | PASS |
| Checkbox action | NO ACTION EXPOSED in fixture | PASS (`Toggle`) | PARTIAL: anonymous actions only |
| List | PASS; parent `Selection` | PASS; `Table` interface | PARTIAL: HTML select is `combo box`; popup has `Selection` |
| ListItem selection | PASS through parent `Selection.select_child` | PASS through item `Toggle` | NOT TESTED |
| Selection backend | PASS | N/A for fixture strategy | NOT TESTED |
| Menu inspection | NOT TESTED in bundled fixture | PASS | PASS for HTML select popup |
| OpenMenu | NOT TESTED | PASS (`ShowMenu`) | NOT TESTED |
| MenuItem activation | NOT TESTED | PASS (`Press`) | NOT TESTED |
| Browser tree | N/A | N/A | PASS: small fixture 277 printed nodes |
| Browser large tree | N/A | N/A | PASS: up to 5,152 nodes |
| Browser object churn | N/A | N/A | PASS: old locator became stale, new locator changed |
| Keyboard TUI loop | PASS | PASS | N/A |
| Mouse TUI loop | PASS (automated SGR input) | PASS (automated SGR input) | N/A |
| Application-gone handling | PASS | PASS | NOT TESTED |

## Qt accessibility activation

In the headless Xvfb session, Qt did not appear in the registry while both `org.a11y.Status` properties were false. The test set:

```bash
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
  --method org.freedesktop.DBus.Properties.Set org.a11y.Status IsEnabled '<true>'
gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
  --method org.freedesktop.DBus.Properties.Set org.a11y.Status ScreenReaderEnabled '<true>'
```

After restarting the Qt fixture, `GetAll org.a11y.Status` returned both properties as true and `gui2tui-inspect --list` reported `gui2tui-qt-fixture`.

## Measured snapshot samples

| Application | Printed nodes | Inspector wall time |
| --- | ---: | ---: |
| GTK4 fixture | 13 | 0.049 s |
| Qt6 fixture | 19 | 0.050 s |
| gtk4-demo initial window | 91 | 0.144 s |
| Chrome browser fixture | 277 | 0.332 / 0.335 / 0.358 s (min/median/max) |
| Chrome large fixture, 25 generated rows | 427 | 0.455 / 0.485 / 0.490 s |
| Chrome large fixture, 100 generated rows | 952 | 1.033 / 1.076 / 1.098 s |
| Chrome large fixture, 250 generated rows | 2,002 | 2.152 / 2.238 / 2.263 s |
| Chrome large fixture, 700 generated rows | 5,152 | 5.925 / 5.929 / 5.941 s |

The GTK/Qt entries are single observations, while every Chrome scale row is three complete
inspector traversals. These are development measurements rather than a benchmark. No transient
object error was printed during the recorded scale runs. Chrome's roughly linear multi-second
cost above 2,000 nodes and verified locator churn are evidence for opening the event-cache design
gate; no event cache is implemented in this phase.

## Browser environment and caveat

The probe used official Google Chrome stable 152.0.7977.64 (`google-chrome-stable` package
152.0.7977.64-1, arm64) on the same Xvfb session, launched as an unprivileged user with
`--force-renderer-accessibility=complete`. The sandbox remained enabled; `--no-sandbox` was not
used. See [browser-probe.md](browser-probe.md) for commands, dynamic-tree results, and the action
compatibility caveat.
