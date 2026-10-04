# GUI2TUI v1.1 FINAL INTEGRATION & RELEASE READINESS HANDOFF

## Final status

`V1.1 FINAL INTEGRATION & RELEASE READINESS VALIDATED — V1.1.0 RC READY`

The exact e8f547e RC package, environment matrix, live large-document
measurement, Firefox repeat and bounded soak completed without a product P0 or
P1. No tag, push, publication or v1.0 artifact mutation was performed.

## Historical completion-pass result before autonomous remediation

`V1.1 FINAL INTEGRATION NOT YET VALIDATED` at that checkpoint

Attempt 1 stopped at the release metadata gate. The existing release workflow
checks `docs/release-notes-v${version}.md`; RC version `1.1.0-rc.1` therefore
required `docs/release-notes-v1.1.0-rc.1.md`, which was absent from `2b8a23f`.

The metadata repair added that RC-specific note without changing runtime,
semantic, installer or native-input behavior. The version gate then passed.
Qualification resumed from the new source, built both exact-RC archives and
passed their archive/ABI checks, but was stopped when the ARM64 installed
package smoke exposed a supported external-text interaction failure. No
runtime fix was made during this pass.

## Git

- Starting HEAD: `de7504a` (`docs: define v1.1 native input capability contract`)
- Behavioral qualification HEAD: `bbae179` (`fix: stabilize v1.1 integration and observation`)
- Invalidated RC source HEAD: `2b8a23f` (`chore: prepare v1.1.0-rc.1 candidate`)
- Repaired RC source HEAD: `63b8ebf` (`chore: complete v1.1.0-rc.1 release metadata`)
- Final docs HEAD: `bc25b30` (`docs: record v1.1 final integration readiness`)
- Branch: `v1.1`
- Worktree: clean before this handoff; ignored `artifacts/` remains external evidence
- Push: not performed; no upstream push was attempted
- Tags: no `v1.1.0` or `v1.1.0-rc.1` tag; immutable `v1.0.0` unchanged

The repaired candidate contains the RC release-note metadata only beyond
`2b8a23f`. `git diff bbae179..63b8ebf -- src runtime scripts .github` is
empty; runtime, semantic, installer and native-input deltas remain zero.

## Production delta: v1.0 to v1.1

The v1.1 implementation retains the v1.0 runtime authority model and adds:

- Semantic Surface region preservation and wrapper normalization;
- bounded Surface content with Reader as a separate continuous-content
  projection;
- capability-driven complete-text qualification and authoritative readback;
- source, edit, focus and readback target separation;
- explicit Semantic Submit versus explicit Raw Enter;
- X11 NativeInputBackend delivery only after current session, generation,
  scope, ticket, target, window and exact Accessibility focus verification;
- bounded, relevant post-operation observation;
- no application-name, window-title or toolkit-specific production branch.

The final stabilization fix in `bbae179` qualified complete multiline editing
before inline Surface materialization and allowed the generic `EditComplexText`
capability to reach the existing external-edit operation. This fixed a real
Surface presentation mismatch: a valid multiline TextInput had been rendered
as a Field, while the external-edit key path still required the obsolete
DocumentSummary shape.

## Final runtime architecture

```text
TUI input
  -> SceneElementId / SceneBinding
  -> UiIntent / SemanticOperation
  -> RuntimeSession + ApplicationGenerationId + InteractionScope
  -> OperationTicket
  -> fresh BackendLocator and current capability/action resolution
  -> AT-SPI Action / EditableText / bounded NativeInputBackend
  -> authoritative readback or relevant bounded observation
  -> refreshed Semantic Surface
```

`NativeInputDelivery` remains a delivery implementation, not a second
authority system. `Semantic Submit` requires a fresh compatible public action;
it never falls back to Enter. `SendKeyEnter` is an explicit low-level intent
and reports delivery plus observed consequence, never `SubmitSucceeded`.

## Semantic Surface

- Meaningful Toolbar, Navigation, Form, Main, Document, Section, Article,
  List, Sidebar, Footer and Dialog boundaries are retained where public
  semantics support them.
- Generic implementation wrappers may collapse only when they add no grouping,
  capability or relationship; multi-child semantic groups remain conservative.
- Diagnostics such as unavailable text, empty and unsupported are not ordinary
  content.
- Surface and Reader share the same semantic content source. Surface uses a
  bounded structural preview; Reader owns continuous text, outline, search and
  paging.
- Existing tests cover bounded document-body compression and Surface/Reader
  source sharing. A dedicated 1k–6k-node live large-document measurement was
  not completed in this phase: release gate BLOCKED pending that evidence.

## v1.0 regression matrix

| Task | Result | Evidence |
| --- | --- | --- |
| Application selection | PASS | Current Managed qualification |
| Structured Form | PASS | Current semantic operation/readback smoke and regression tests |
| Button / Toggle / CheckBox | PASS | Current regression suite; historical live evidence |
| Single Selection | PASS | Current regression suite; historical live evidence |
| Table row | PASS WITH EXISTING SAFE LIMITATION | Current semantic tests; provider-specific public semantics remain authoritative |
| PageTab | PASS | Current regression suite |
| Hierarchy | PASS | Current regression suite |
| Menu / Dialog / Modal | PASS WITH EXISTING SAFE LIMITATION | Current scope tests and historical live evidence |
| Open File / Choose Folder | PASS WITH EXISTING SAFE LIMITATION | Historical v1.0 qualification; not re-run in the exact RC package |
| Reader | PASS WITH EXISTING SAFE LIMITATION | Current Reader tests and historical evidence; large live measurement pending |
| Value | PASS | Current regression suite and managed semantic smoke |
| Single-line Text | PASS | Current edit/readback tests and managed smoke |
| Qualified complex Text | PASS | Current Managed qualification after `bbae179` |
| Dynamic continuation | PASS | Current Managed qualification |
| Stale authority refusal | PASS | Current Managed qualification and runtime tests |
| Application replacement / fresh reselection | PASS | Current Managed qualification |
| PasswordText safety | PASS | Current unit tests and historical safety evidence |
| External text handler | PASS | Current Managed qualification |
| Clean terminal exit | PASS | Current Managed qualification |

No v1.1 regression was observed in the completed source checks. Rows marked
with limitations retain the original provider/topology boundaries; they are
not claims of broader application support.

## Native Input

### Submit contract

Fresh public Accessibility evidence and a fresh advertised action are required.
If no compatible action exists, Semantic Submit is `Unsupported`. Direct action
delivery with no relevant consequence is `DeliveredButUnconfirmed` or
`NoDetectableChange`; it never invokes Raw Enter.

### Raw Enter contract

The explicit Raw Enter path is:

```text
current UiIntent::SendKeyEnter
  -> runtime/generation/scope/ticket authority
  -> fresh target and owning-window resolution
  -> window activation and verification
  -> exact Accessibility focus request and verification
  -> final authority check
  -> X11 NativeInputBackend Enter
  -> bounded relevant observation
```

Controlled fixture evidence from the behavioral source passed Edit, Raw Enter,
retired-generation refusal, wrong-focus refusal, scope refusal and multiline
Submit-versus-Raw-Enter distinction. Unit tests also confirm that failed
authority, activation or focus gates do not call the keyboard backend.

### Edit contract

AT-SPI mutation is preferred. Every text mutation performs fresh authoritative
readback. Native text fallback is allowed only for verified EditText under the
same authority chain and only accepts printable ASCII in this candidate. A
readback mismatch yields `EditNotVerified`; no automatic Submit or Raw Enter
follows.

## Firefox signature E2E

The validation-only ASCII Page A/Page B run from the prior v1.1 authority
qualification produced:

```text
EDIT_VERIFIED       PASS
SEMANTIC_SUBMIT     UNSUPPORTED (no compatible public action)
RAW_ENTER_DELIVERY  PASS
DOCUMENT_CHANGED    PASS
FRESH_SURFACE_PAGE_B PASS
```

Discovery was generic: an editable/focusable ComboBox exposing EditableText,
Text and Component, with no advertised actions. No browser name, title, URL or
DOM logic exists in production. This evidence is valid for the behavioral
source and the RC source has no runtime change after `bbae179`; however, an
exact RC archive was not built, so the required installed-RC signature smoke is
still a release blocker. A 5–10 cycle dynamic repeat was not completed.

## Observation and performance

Observation remains bounded, uses the normal Surface refresh pipeline, and
distinguishes delivery from relevant change, stale target, replacement,
value/state change, timeout and refresh failure. Existing tests cover immediate
and delayed changes, document replacement, stale-after-delivery,
unrelated-application churn and refresh failure.

The default observation window remains approximately three seconds with early
return on a relevant consequence. A final candidate run recording poll count,
reload count and traversal time for small, Firefox and large trees was not
completed. No unbounded polling path was found in source review.

## Runtime stability and environments

- Managed Xvfb: PASS on `bbae179`, including install, Doctor, application
  selection, dynamic refresh, semantic readback, terminal navigation, command
  palette, reconnect, stale binding refusal, external handler and safe
  uninstall.
- Docker interactive PTY and same-host SSH: current requalification BLOCKED by
  the local Docker arm64 base-image dependency download; no failure of the
  product path was observed.
- Wayland/XWayland: historical v1.0 Weston evidence remains applicable as
  topology evidence, but current exact-RC requalification was not completed.
- Native aarch64 package: direct Orb VM packaging was rejected by the required
  ABI gate (`GLIBC 2.39 exceeds 2.35`), so it is not release evidence.
- Native x86_64: not available; no native qualification claim.
- 30–45 minute v1.1 resource soak: not completed. No new monotonic-resource
  evidence is claimed.
- Terminal sizes 48x160, 24x72, 16x60 and 12x50: existing renderer tests and
  historical qualification cover layout safety; exact-RC interactive matrix
  was not rerun.

## Security and genericity audit

- Genericity search found no application-specific actuation branch. Firefox,
  Chromium and launcher names occur only in launcher/catalog tests or generic
  application-list handling; `window_title` is presentation text, not routing.
- XTEST and `x11rb` occur in `src/tui/actuation.rs` as the NativeInputBackend;
  terminal mouse input and spatial coordinates remain presentation/TUI input,
  not GUI pointer injection.
- No uinput, coordinate click, native mouse injection, DOM/CDP, backing-file
  mutation or arbitrary key scripting path was found.
- Runtime tests cover ticket retirement, generation replacement, scope
  rejection, stale locator rejection and late-result suppression.
- PasswordText remains unread, uncached, unrendered and excluded from external
  editing; the native fallback does not read password content.
- No wrong-target, stale-generation, unverified-focus or silent Submit-to-Enter
  injection was observed.

## Candidate packages

No exact RC archives were produced. Consequently the following fields are
intentionally `NOT BUILT`, not guessed:

| Architecture | Archive | Source | Size/SHA256 | ABI/install |
| --- | --- | --- | --- | --- |
| aarch64 | `gui2tui-1.1.0-rc.1-linux-aarch64.tar.gz` | NOT BUILT | NOT AVAILABLE | NOT QUALIFIED |
| x86_64 | `gui2tui-1.1.0-rc.1-linux-x86_64.tar.gz` | NOT BUILT | NOT AVAILABLE | NOT QUALIFIED; native hardware unavailable |

The RC source version is present in `2b8a23f`, but candidate immutability and
archive hashes cannot be claimed until both package builds and fresh installs
complete.

## Documentation and quality matrix

- README, Getting Started, Deployment, Limitations and Troubleshooting now
  document Surface/Reader, verified Edit and explicit Raw Enter boundaries.
- Architecture and AGENTS contracts retain the separation of Intent,
  Authority, Delivery and Observation.
- `cargo fmt --all -- --check`: PASS
- `cargo check --all-targets --locked`: PASS
- `cargo test --all-targets --locked --no-fail-fast`: PASS — 339 library, 2
  inspector and 5 CLI tests
- `cargo clippy --all-targets --locked -- -D warnings`: PASS
- `python3 scripts/check-docs.py`: PASS (`DOCUMENT_AUDIT=PASS`, 106 files,
  332 local links)
- `git diff --check`: PASS

## Severity

- P0: none observed.
- P1: exact-RC package/install matrix, current Docker/SSH and Wayland
  qualification, required large-document measurement, dynamic Firefox repeat
  and bounded resource soak are incomplete. Therefore RC readiness is blocked.
- P2: native x86_64 hardware, Wayland Raw Enter without X11 backend,
  printable-ASCII native text fallback and ordinary desktop/VT matrices remain
  explicit limitations.

## Release state

- Package version in RC source: `1.1.0-rc.1`
- `v1.1.0-rc.1` tag: absent
- `v1.1.0` tag: absent
- GitHub Release: absent
- Push: not performed
- `v1.0.0` tag and release assets: unchanged

## Minimal next gate

Do not publish this candidate. To reach RC readiness, first run the exact
candidate package pipeline in an environment with the qualified Ubuntu 22.04
glibc ≤2.35 builders, then run fresh package install/upgrade, Docker/SSH,
Wayland/XWayland, the large-document measurement, repeated Firefox local-page
cycles and the bounded resource soak. Any runtime change requires a new
behavioral source and a new RC candidate.

## Completion pass: RC metadata repair and qualification resume

### Repair and provenance

- Previous blocker: exact-version release-note metadata mismatch.
- Repair commit: `63b8ebf` (`chore: complete v1.1.0-rc.1 release metadata`).
- RC note added: `docs/release-notes-v1.1.0-rc.1.md`.
- Version gate: `RELEASE_VERSION_GATE=PASS version=1.1.0-rc.1`.
- `git diff bbae179..63b8ebf -- src runtime scripts .github`: empty.
- `RUNTIME_DELTA=0`, `SEMANTIC_DELTA=0`, `INSTALLER_BEHAVIOR_DELTA=0`,
  `NATIVE_INPUT_DELTA=0`.
- `v1.1.0-rc.1` and `v1.1.0` tags remain absent; no push, publication or
  release upload was performed.

### Exact RC package evidence

Both archives were built from `63b8ebf` in the existing Ubuntu 22.04
qualified-container baseline with Rust/Cargo 1.88 and locked dependencies.
Archive safety validation and the per-architecture ABI validator passed.
The package output is external evidence under `/tmp`, not a tracked release
artifact.

| Arch | Archive | Bytes | SHA256 | Binary hashes (`gui2tui`, `inspect`, `local`) | ABI |
| --- | --- | ---: | --- | --- | --- |
| aarch64 | `gui2tui-1.1.0-rc.1-linux-aarch64.tar.gz` | 16,030,102 | `4b822bf751db8ba0758a9c788ad33b3b612e00fa20a333e522aed1f545985dbc` | `c9b8f7d1fc79cd7d000c9d638a46d421fc1b9accd9f8b1f99b712e449480b6c2` / `8f53814669cabc5189e2cea73688db94314859c3223e16a8344023a408027e79` / `7a14ebc9df4bc8fb317df70f92a1d065c54d240598fbea7a170e582a4a118424` | AArch64, glibc 2.34, no GLIBCXX |
| x86_64 | `gui2tui-1.1.0-rc.1-linux-x86_64.tar.gz` | 16,237,817 | `348de97ee2d318babeec716a00712a0fa81784f969b44099597168e3d9504dea` | `2588875e28cfb1fefc05d13905a69be1966cb5e671876b424ce99bb234d8f073` / `f79cee91a58cdc0760fc76b28a266db1547880cb42c0bebde5328d9469087a8e` / `3c83650d9651f94733993ccbc4b9bafa35bfb63a3a9c30f1b3861b9a0eab0201` | AMD64, glibc 2.34, no GLIBCXX |

Common metadata: `Cargo.lock` SHA256
`0ee61c5901613f4877ff1b23752affbd218324dc5285a97b1bb545e389600165`;
`BUILD-INFO.commit=63b8ebf`; `BUILD-INFO.version=1.1.0-rc.1`.
The x86_64 result is emulated/container evidence only; native x86_64 remains
unqualified.

### Exact-RC installation result and stopping blocker

The ARM64 archive was extracted and installed in the qualified package image.
The following package checks passed before the smoke failure:

- installed `--version` and package metadata resolution;
- Doctor session-bus, AT-SPI absence, helper permissions and diagnostics;
- Managed setup/stop and descriptor safety checks;
- interactive terminal checks and invalid-terminal rejection paths;
- safe uninstall preconditions and package-owned helper layout.

The installed packaged semantic smoke then failed at the complete-text
workflow while waiting for `Edit externally` in the extracted-package TUI.
The captured frame exposed the external text control and its GUI controls, but
the expected external-edit operation was not reachable through the packaged
surface before the bounded wait expired. This is a candidate product/regression
blocker for a v1.0-qualified external-text task, not an infrastructure or
metadata failure. Qualification stopped at this point as required; no runtime
code was changed to work around it.

Result: `EXACT_RC_FRESH_INSTALL=BLOCKED`;
`PRODUCT_P1_CANDIDATE=packaged external-text interaction not reachable`.
The x86_64 install smoke, custom-prefix replay, v1.0 public-artifact upgrade,
Docker/SSH/Wayland exact-RC runs, Firefox repeat, live large-document
measurement, observation metrics and 30–45 minute soak were not run after this
stop and are not claimed as PASS.

### Gate status after repair

| Gate | Result |
| --- | --- |
| RC exact-version metadata | PASS |
| aarch64/x86_64 package build | PASS (x86_64 emulated) |
| ABI and archive audit | PASS |
| aarch64 fresh install | BLOCKED by packaged external-text smoke |
| x86_64 emulated install | NOT RUN after blocker |
| Firefox exact-RC signature/repeat | NOT RUN after blocker |
| large-document measurement | NOT RUN after blocker |
| observation measurement | NOT RUN after blocker |
| Docker / SSH / Wayland / XWayland | NOT RUN after blocker |
| resource soak | NOT RUN after blocker |
| full exact-RC quality matrix | NOT RUN after blocker |

### Severity and release state

- Product P0: 0 observed.
- Product P1: 1 candidate blocker — packaged external-text interaction did not
  reach `Edit externally` in the exact-RC smoke.
- Qualification blockers: package qualification cannot continue until that
  behavior is triaged and, if confirmed as a product defect, fixed in a new
  behavioral source and rebuilt RC.
- Current status remains: `V1.1 FINAL INTEGRATION NOT YET VALIDATED`.
- No tag, push, GitHub Release or public upload was performed.

## Final qualification completion pass: e8f547e

The earlier blocker chronology is retained above. The bounded autonomous
remediation then proceeded as follows:

1. `63b8ebf` repaired the exact-version RC release-note gate.
2. `5d977c9` repaired the generic qualified external-text reachability path.
3. `a7c5be9` preserved nested editable controls in the normalized Surface.
4. `21f4c52` and `e74c627` repaired generic WM-less X11 window resolution and
   missing optional X11 property handling.
5. `e8f547e` bounded unbound document preview in the ordinary Surface while
   preserving interactive bindings and the full Reader projection.

The installed-RC external-text timeout was diagnosed as a validation-path
failure: the probe waited for styled terminal transcript text (`Raw Enter` or
`delivery`) whose characters can be split/elided during cursor-addressed
redraw. The actual status was `succeeded`/`DocumentChanged`. The probe was
changed to wait for stable semantic status and, for repeat/soak, to verify the
old page before Edit and the target page after Raw Enter. No production or
package behavior was changed for this harness correction.

### Candidate identity and package provenance

- Behavioral qualification source: `bbae179`.
- Final behavioral/RC source: `e8f547e9c804d358c706cadda0247080d983d51d`.
- Candidate version: `1.1.0-rc.1`.
- Builder baseline: existing Ubuntu 22.04 qualified containers, locked
  dependencies, Rust/Cargo 1.88. No new builder was introduced.
- `Cargo.lock` SHA256:
  `0ee61c5901613f4877ff1b23752affbd218324dc5285a97b1bb545e389600165`.
- The archives below are external evidence under `/tmp`; they are not tracked
  release artifacts.

| Arch | Archive / bytes | Archive SHA256 | `gui2tui` / `inspect` / `local` SHA256 | ABI |
| --- | ---: | --- | --- | --- |
| aarch64 | `gui2tui-1.1.0-rc.1-linux-aarch64.tar.gz` / 16,035,440 | `b55aec49d326adf63e3f916cffee130292cb96c90493ba8a0dfc1a562d505bad` | `6e68366380762799d21da5d2ded1dfbebe6a7790869564517ccf9cac11eb716f` / `870a935005002dd04d59566e94ec92cee2eb0d09fedec9e1d4c1e2598035180a` / `9e32b51d54168d173fb55541cd4efaea9b1272d5d3793be00a101d1a77538301` | AArch64, `/lib/ld-linux-aarch64.so.1`, glibc 2.34, no GLIBCXX |
| x86_64 | `gui2tui-1.1.0-rc.1-linux-x86_64.tar.gz` / 16,250,915 | `99527eacf85b872551a47072a8360ce16db1772cfe30e5846be3cba9435f332a` | `6d0536a979a3a2563509e38ad8c4e7dd2385a68a1e5647011d938dd22d472c0e` / `f213716735c8f1dfc54a9550ee91137221b8a14112c7c87a209a3e216333fe60` / `9543a3ec85f0d1cedc0e4ded0838e9ad278752caa66b9f0e0b4c27915c267189` | AMD64, `/lib64/ld-linux-x86-64.so.2`, glibc 2.34, no GLIBCXX |

Both archive validators, manifest/checksum consistency and the `glibc <=
2.35` gate passed. Native x86_64 hardware was unavailable; its runtime result
is emulated/container evidence only.

### Exact-RC package and environment qualification

- aarch64 fresh install: `PASS`; installed `--version`, Doctor, Managed
  setup/stop, helper resolution, semantic package smoke and safe uninstall all
  passed. Custom absolute prefix containing spaces also passed.
- x86_64 package/fresh-install smoke: `PASS` under the existing emulated
  Ubuntu 22.04 baseline; native x86_64 remains unqualified.
- Public v1.0.0 aarch64 archive was obtained from the immutable v1.0.0
  release and the documented replacement replay passed: preserved config
  marker and unrelated prefix file survived v1.0.0 uninstall and exact-RC
  install/uninstall.
- Docker interactive PTY and same-host SSH → Managed: `PASS`, including
  semantic readback, external handler, disconnect boundary and clean exit.
- Native Wayland: `QUALIFIED_WITH_EXPLICIT_LIMITATIONS`; semantic Surface and
  public semantic operations passed, while native Raw Enter remains capability
  dependent. XWayland: `QUALIFIED_WITH_EXPLICIT_LIMITATIONS` under the existing
  headless validation boundary.

### Exact-RC Firefox signature and repeat

Using the installed aarch64 exact-RC binary and ASCII-only local Page A/B:

```text
EDIT_VERIFIED=PASS
SEMANTIC_SUBMIT=UNSUPPORTED
RAW_ENTER_DELIVERY=PASS
DOCUMENT_CHANGED=PASS
FRESH_SURFACE_PAGE_B=PASS
```

The generic target was the editable/focusable Accessibility control; no
browser, title, URL or DOM branch exists in production. Five bounded cycles
`A→B→A→B→A` passed with the revised semantic probe. The probe records a
`NoDetectableChange`/`Timeout` as a real observation outcome when the relevant
state is not seen; it does not convert it to Submit success.

### Large document and observation measurements

The live generic document fixture contained 6,842 accessible nodes and 6,006
Reader blocks. On e8f547e:

```text
bootstrap/cache:       185–204 ms
region acquisition:    321 ms
scene acquisition:     289 ms
content projection:    254 ms
Surface elements:      2,816 (2,427 interactive)
Reader blocks:          6,006
content compression:   6,324 -> 2,816 elements
```

The Surface retained the 2,426 bound interactive descendants plus the
structural summary/preview; the full body remained in Reader. No fabricated
nodes or unbounded traversal was observed. Observation uses the normal
refresh pipeline with a 3,000 ms deadline and 40 ms poll interval, returning
immediately on relevant change. Unit coverage verifies immediate, delayed,
stale, replacement, value/state, unrelated-churn and timeout behavior. Live
Firefox runs observed the relevant consequence within the bounded window;
the current user-facing trace does not expose the internal poll counter, so
poll/reload counts are not claimed beyond the bounded policy and early-stop
result.

### Terminal, security and quality gates

- Exact-RC terminal smoke: `48x160`, `24x72`, `16x60`, `12x50` all `PASS`;
  resize/quit did not inject input or mutate the target.
- Genericity audit: no production application-specific actuation branch;
  Firefox/Chromium strings are limited to validation/catalog tests or generic
  launcher examples. `Submit` has no Raw Enter fallback.
- Injection audit: XTEST appears only in the X11 NativeInputBackend;
  native mouse/coordinate injection and uinput are absent.
- Authority audit: native delivery remains behind session, generation,
  scope, ticket, fresh locator, owning-window and exact-focus checks. Existing
  negative tests confirm stale, scope, activation and focus refusal without a
  keyboard call.
- PasswordText remains unread, uncached and unlogged.
- `cargo fmt --all -- --check`: `PASS`.
- `cargo check --all-targets --locked`: `PASS`.
- `cargo test --all-targets --locked --no-fail-fast`: `PASS` — 343 library,
  2 inspect and 5 CLI tests.
- `cargo clippy --all-targets --locked -- -D warnings`: `PASS`.
- `python3 scripts/check-docs.py`: `PASS` (`108` files, `336` local links).
- `git diff --check`: `PASS`.

### Resource soak

The final single-process soak used the installed exact-RC aarch64 package, the
local ASCII fixture, sequential Surface/Edit/Raw Enter/observation cycles and
periodic Firefox RSS/FD/thread samples. It ran 86 batches from `18:39:18` to
`19:11:29` (32 minutes 11 seconds); every batch passed. Each batch contained
five alternating page cycles, for 430 successful edit/raw-key observation
workflows.

```text
SOAK_CYCLES=86
SOAK_FAILURES=0
SOAK_TOTAL_BATCH_SECONDS=1929
SOAK_AVERAGE_BATCH_SECONDS=22.43
FIREFOX_RSS_KB=256208..355936
FIREFOX_FD=123..130
FIREFOX_THREADS=87..95
```

No monotonic FD or thread growth, zombie accumulation, failed operation
ticket cleanup or unbounded artifact growth was observed. The detached probe
attempts that preceded this run were discarded because they had concurrent
processes; only this single-process run is qualification evidence.

### Final gate state

| Gate | Result |
| --- | --- |
| aarch64 exact package / ABI / archive | PASS |
| x86_64 exact package / ABI / archive | PASS (emulated runtime) |
| fresh install / custom prefix / v1.0 replacement | PASS |
| Firefox exact-RC signature / five-cycle repeat | PASS |
| large document measurement | PASS |
| Docker / SSH / Wayland / XWayland | PASS with declared limitations |
| terminal matrix | PASS |
| observation bounds | PASS; internal poll count not externally surfaced |
| resource soak | PASS — 86 batches / 32m11s / 0 failures |
| security and genericity | PASS |
| source quality matrix | PASS |
| Product P0 | 0 |
| Product P1 | 0 |
| Qualification blockers | 0 |

Release actions remain forbidden: no tag, push, GitHub Release or public
upload was performed; `v1.0.0` remains unchanged.

## Final release state

- Behavioral qualification source: `bbae179`.
- RC source: `e8f547e9c804d358c706cadda0247080d983d51d`.
- Final documentation HEAD: the docs-only commit containing this definitive
  HANDOFF.
- Branch: `v1.1`.
- Worktree after the docs commit: clean.
- `v1.1.0-rc.1` tag: absent.
- `v1.1.0` tag: absent.
- GitHub Release/public upload: absent.
- Push: not performed.
- `v1.0.0` tag and release assets: unchanged.

Final status:

`V1.1 FINAL INTEGRATION & RELEASE READINESS VALIDATED — V1.1.0 RC READY`
