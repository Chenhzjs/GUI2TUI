# Compatibility matrix

Validated on 2026-08-28 in Ubuntu 24.04 arm64, Xvfb/X11, AT-SPI 2.52, GTK 4.14.5, and Qt 6.4.2 through PyQt 6.6.1.

| Feature | GTK4 | Qt6 | Browser |
| --- | --- | --- | --- |
| Application discovery | PASS | PASS | NOT TESTED |
| Button role | PASS (`button`) | PASS (`button`) | NOT TESTED |
| Button action | PASS (`Click`) | PASS (`Press`) | NOT TESTED |
| Plain TextInput | PASS (`text`) | PASS (`text`) | NOT TESTED |
| Password TextInput | PASS (`password text`) | PASS (`password text`) | NOT TESTED |
| Password redaction | PASS | PASS | NOT TESTED |
| Checkbox state | PASS | PASS | NOT TESTED |
| Checkbox action | NO ACTION EXPOSED in fixture | PASS (`Toggle`) | NOT TESTED |
| List | PASS; `Selection` interface | PASS; `Table` interface | NOT TESTED |
| ListItem | PARTIAL; `listitem.scroll-to` is not selection | PASS; `Toggle` set `selected` | NOT TESTED |
| Menu inspection | NOT TESTED in fixture | PASS for tree discovery | NOT TESTED |
| Menu TUI navigation | NOT IMPLEMENTED | NOT IMPLEMENTED | NOT IMPLEMENTED |
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

These are single observations in the Xvfb VM, not benchmarks. The TUI reported initial snapshot times of 18–36 ms for the GTK fixture and 33–48 ms for the Qt fixture during interactive runs. Browser probing was not performed because no Chromium, Chrome, or Firefox executable was installed.
