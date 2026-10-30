# Public state and first-use TUI validation — 2026-10-30

This development-source check covers public state acquisition, shared names,
terminal navigation and first-use guidance. It does not create a binary release.

## Current behavior

- A choice owner's public Selection members are read and matched to exact
  available objects. Missing child Selected flags do not hide a reported current
  value. None selected, multiple members, failed reads and unresolved members
  remain distinct. These reads do not grant operation authority.
- Human F3 and Agent operation labels share public name, region, description and
  explicit content-window context. Raw names, descriptions and label provenance
  remain available. A window name is not presented as a document's own title.
- Successful empty field reads are distinct from unread/unexposed/failed values.
  Property-read failures survive node refresh. Reader keeps known labels while
  marking unavailable public text and reporting partial reads.
- Public content is not suppressed simply because its text says Unknown,
  Empty or Unsupported. Unknown native roles remain identifiable by their
  public role name; no capability is inferred from that wording.
- The selector, main-view footer and contextual help introduce the short path:
  choose an application, Tab to a control, Enter to use, F3 to find an action,
  Esc to close/cancel, F1 for help. [English](../first-steps.md) and
  [中文](../first-steps.zh-CN.md) walkthroughs need no model or API key.

## Final checks

Linux aarch64 container, private Xvfb/session D-Bus/AT-SPI:

| Scene | Public operations retained | Verified behavior |
| --- | ---: | --- |
| FeatherPad | 134 | Public descriptions name 13 formerly unnamed operation targets; F3 displays Clear text; document text and all operation parameters retained. |
| L3afpad | 122 | Reader, search, region navigation and F3 retain public content and operations. |
| Synthetic form | 25 | Current choice is Draft, backed by public Selection; local choice browsing and cancellation work. |
| Synthetic modal dialog | 2 | Modal interaction scope and narrow help/content remain confined. |

For the same GUI in each scene, before/after public operations and parameter
domains, full Text payloads and payload references were checked. Window resizing
did not alter these facts. Independent public AX reads were unchanged, with no
read errors. The two synthetic scenes are not additional real applications.

A separate public Selection probe confirmed Draft through the same exact raw
member before and after G2T observation. A separate first-use terminal walkthrough
verified application selection, F1 help, the visible help hint at 40 columns,
F3 name search and cancelling a locally edited field.

`cargo test --all-targets`: **462 passed** (454 library tests).
`cargo clippy --all-targets -- -D warnings`, formatting, patch whitespace and
Human/Agent builds passed. GUI writes and paid decision API requests: **0**.

## Practical limits

This is engineering evidence, not a human user study or a measured improvement
in task success or efficiency. Broader application coverage, actual user
onboarding and provider/transport fault injection remain to be evaluated.
Unexposed or virtualized content can still be partial. Unknown effects never
become automatic retries or successful task completion.

The optional F7 console requires a compatible separately installed Jev console
client. The currently pinned Jev submodule does not supply that console command;
normal Human TUI, F3 and Reader do not depend on it. See [Jev setup](../jev-client.md).
