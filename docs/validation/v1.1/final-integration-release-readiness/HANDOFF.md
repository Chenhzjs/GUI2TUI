# GUI2TUI v1.1 FINAL INTEGRATION & RELEASE READINESS HANDOFF

## Final status

`V1.1 FINAL INTEGRATION NOT YET VALIDATED`

The behavioral source is healthy and the v1.1 architecture is preserved, but
this phase cannot claim RC readiness because the exact RC package matrix,
current Docker/SSH and Wayland qualification, large-document measurement and
the required bounded soak were not completed in this environment. No tag,
push, publication or v1.0 artifact mutation was performed.

## Git

- Starting HEAD: `de7504a` (`docs: define v1.1 native input capability contract`)
- Behavioral qualification HEAD: `bbae179` (`fix: stabilize v1.1 integration and observation`)
- RC source HEAD: `2b8a23f` (`chore: prepare v1.1.0-rc.1 candidate`)
- Final docs HEAD: `bc25b30` (`docs: record v1.1 final integration readiness`)
- Branch: `v1.1`
- Worktree: clean before this handoff; ignored `artifacts/` remains external evidence
- Push: not performed; no upstream push was attempted
- Tags: no `v1.1.0` or `v1.1.0-rc.1` tag; immutable `v1.0.0` unchanged

The two release-candidate source commits are intentionally separate: runtime
and documentation stabilization is in `bbae179`; the RC version mutation is
the version-only child `2b8a23f`.

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
