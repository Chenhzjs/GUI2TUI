# Your first few minutes with GUI2TUI

Human TUI use needs no model or API key. GUI2TUI runs against Linux public
Accessibility; a macOS build does not supply a Linux desktop session. Follow
[Getting started](getting-started.md) for installation and Managed setup.

## Open an application

For an application already running in your Linux desktop session:

```sh
gui2tui --session desktop doctor
gui2tui --session desktop
```

Choose the application with the arrow keys and Enter. `/` filters a long list.
If the list is empty, start the application in that same session and press `r`.
Press `d` to check the connection. For a Managed session, use `--session managed`
after the setup steps in Getting started; applications in a different session
will not appear here.

## Start with five keys

| Key | What happens |
| --- | --- |
| Tab / Shift-Tab | Move the highlighted control in the current pane. |
| Enter | Use it, open its text editor, show its choices, or read its document. |
| F3 | Search all public actions in the current interaction scope by name. Type a name, use arrows to choose, then Enter. |
| Esc | Close a panel or cancel local editing. From the main view, Esc quits. |
| F1 | Open help for the panel you are using. Up/Down scroll help; Esc returns. |

Use `F6` to change regions when Tab does not reach the part you need. In a
region with several panes, `Ctrl-Tab` changes panes. You can leave a panel and
return without changing the application.

Try this first: select a text field, press Enter, type a small change, then
press Esc. The local edit is cancelled. To apply an intended change, use
`Ctrl-S`; the GUI's readback must confirm it. Plain Enter may also request
submission when the application explicitly exposes it, so Ctrl-S is the
clearest way to apply text only. In an editor, q and ? are text; use F1 for help.

For a document, Enter opens Reader. `/` searches loaded text; Ctrl-F in the
search panel continues across exposed content. Esc returns to the main view.

## Understand what the interface is telling you

- **Read only / action unavailable:** the highlighted default operation is not
  available. F3 may expose other public operations on that object.
- **No public title / Unnamed:** a name was not supplied. Context can help identify
  an object, but a number or position is not proof of what it does.
- **Not read / read failed / partial:** content is not established as complete.
  Readable content and supported operations elsewhere can still be used.
- **Unknown result:** delivery or effect could not be confirmed. Inspect the
  current view and press `r` to refresh the main view before deciding what to do;
  blindly repeating an action can duplicate an effect.

`b` returns to application selection; `q` or Ctrl-C exits. Optional resources
(F4), workflows (F7) and external editing are described in contextual help;
they are not prerequisites for normal use. Passwords and visual verification
may require operating the original GUI through an authorized human handoff.
