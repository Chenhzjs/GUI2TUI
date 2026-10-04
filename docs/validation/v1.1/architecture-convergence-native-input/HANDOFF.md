# GUI2TUI v1.1 Architecture Convergence & Native Input Contract

## Status

`V1.1 ARCHITECTURE CONVERGENCE & NATIVE INPUT CONTRACT VALIDATED`

This is a local development-branch qualification. It does not create a
release, tag, package, or push. The v1.0 release artifacts remain unchanged.

## Git

- Starting HEAD: `767dd50` (`v1.1` before convergence work)
- Final HEAD: recorded by the final docs commit
- Branch: `v1.1`
- Push: not performed
- Local commits: one implementation commit and one documentation/contract
  commit are intended after all checks pass.
- Existing validation videos and artifacts remain outside the implementation
  commits.

## Final architecture

```text
TUI input
  -> SceneElementId / SceneBinding
  -> UiIntent / SemanticOperation
  -> RuntimeSession + ApplicationGenerationId + InteractionScope
  -> OperationTicket
  -> fresh BackendLocator and current capability/action resolution
  -> AT-SPI Action / EditableText / bounded NativeInput
  -> fresh authoritative readback or relevant bounded observation
  -> refreshed Semantic Surface
```

`NativeInputDelivery` is a low-level backend component. It is not a second
operation authority and is not called as an `app.rs -> independent
ActuationEngine -> XTEST` bypass. The last native-input gate checks the
runtime, generation, scope, exact locator, owning native window, exact
Accessibility focus, and live operation ticket immediately before emission.

### Semantic Submit

Submit requires fresh current public Accessibility evidence and a compatible
advertised action. It invokes the action by current semantic name/identity;
numeric action ordering is not authority. If evidence is absent, Submit is
unsupported. If direct delivery is accepted with no observed consequence, the
trace reports delivery plus `NoDetectableChange`; it never falls back to raw
Enter.

### Explicit Raw Enter

`SendKeyEnter` is an independent explicit low-level intent. The current TUI
surface labels it as raw input (`Alt-Enter` / help text), not Submit. It can
report `Enter delivered` with `Changed`, `DocumentChanged`, `TextChanged`, or
`NoDetectableChange`. It never reports `Submit succeeded`.

### Edit

Edit is separate from Submit. Source, edit, focus, and readback targets are
resolved independently. Accessibility mutation is preferred; every mutation
requires fresh authoritative readback. The X11 native text fallback is bounded
to printable ASCII and is accepted only when readback exactly equals the
requested value. Edit failure cannot trigger automatic Submit.

## AGENTS.md contract change

The former absolute prohibition on keyboard injection was narrowed. The new
normative rule allows only a very small explicit raw-key capability after the
normal authority chain has freshly verified runtime/session, generation,
scope, ticket, target, owning window, and exact Accessibility focus. Native
delivery is not semantic authority; delivery and observation are reported
separately. Semantic Submit may not silently use Raw Enter. Mouse injection,
coordinate clicking, pointer guessing, uinput, arbitrary key sequences, and
unverified replay remain prohibited.

## Surface and Reader

- Region analysis retains Toolbar, Navigation, Search/Form, Main, Document,
  Section, Article, List, Sidebar, Footer, Dialog, and other public semantic
  boundaries.
- Implementation-only wrappers collapse only when safe. Structural multi-child
  groups are retained; direct-control/field wrappers may collapse while their
  bindings survive.
- Surface shows regions, controls, relationships, and a bounded content
  preview (currently capped at 96 preview lines).
- Reader is a separate projection of the same content source and owns
  continuous text materialization, outline, search, and paging.
- Accessibility diagnostics and empty/unknown noise are not ordinary content.

## Controlled fixture qualification

Fixture: `tests/fixtures/native_input_authority_fixture.py`.

- Edit `hello-authority`: PASS — strict authoritative readback.
- Semantic Submit: PASS as a negative contract — direct `Activate` delivery
  was accepted, result remained `not submitted`, observation was
  `NoDetectableChange`, and no native Enter fallback occurred.
- Raw Enter: PASS — window activation verified, exact focus verified, Return
  delivered, fixture result became `submitted:hello-authority`, and the
  observation reported `Changed`.
- Retired generation: PASS — native key gate rejected the operation and the
  recording backend received no key.
- Wrong/failed authority or focus: PASS — the native backend is not called.
- Scope violation: PASS through the authority gate tests; no key is emitted.
- Multiline distinction: PASS — Submit without semantic evidence is
  unsupported, while explicit Raw Enter is independently resolvable.

## Firefox ASCII local E2E

Validation-only pages are in `tests/fixtures/v11_ascii_web/` and are served
by a temporary local HTTP server. Production code does not know their names or
the browser identity.

Fresh public Accessibility discovery found the address control as a generic
editable/focusable `ComboBox` with `EditableText`, `Text`, and `Component`
interfaces and no advertised actions. The exact address value was edited from
Page A to `http://127.0.0.1:<port>/b.html` and was confirmed through fresh
Accessibility readback.

Results from the successful current-worktree run:

```text
EDIT_VERIFIED              PASS
SEMANTIC_SUBMIT            UNSUPPORTED (no compatible public action)
SEMANTIC_SUBMIT_FALLBACK   NOT USED
RAW_ENTER                  PASS
DOCUMENT_CHANGE            PASS (Page A -> Page B surface)
FRESH_SURFACE_PAGE_B       PASS
```

The TUI surface after Raw Enter displayed `PAGE_B`; a fresh public
Accessibility snapshot exposed Window `GUI2TUI Page B`, Document `GUI2TUI
Page B`, and Heading `PAGE_B`. A separate Semantic Submit attempt left Page A
visible and reported unsupported, with no Raw Enter trace.

## Safety results

- Wrong-target injection: not observed; window and exact focus gates refused
  unverified routing.
- Stale-generation injection: not observed; recording backend received no key.
- Unverified-focus injection: not observed; focus verification is a hard gate.
- Silent Semantic Submit fallback: not observed after convergence; the code
  path was removed and the live fixture showed `NoDetectableChange` instead.
- Application-specific production branch: none added.
- Unrelated Accessibility churn does not confirm an operation; observation is
  scoped to the target window/document/subtree and relevant target state.

## Tests and quality matrix

- `cargo fmt --all -- --check`: PASS
- `cargo check --all-targets --locked`: PASS
- `cargo test --lib --locked --no-fail-fast`: PASS, 338 tests
- Existing regression tests: PASS
- `cargo clippy --all-targets --locked -- -D warnings`: PASS
- `python3 scripts/check-docs.py`: PASS (`DOCUMENT_AUDIT=PASS`)
- `git diff --check`: PASS
- Genericity and injection audits: PASS; validation names occur only in test,
  launcher, or fixture contexts.

## Limitations

- Raw input is currently only explicit Enter; it is not an automation or macro
  API.
- Native input backend is X11/XTEST only. Wayland native input is not
  implemented.
- Native text fallback is printable ASCII only; AT-SPI EditableText Unicode
  support remains provider-dependent.
- Mouse/pointer injection, coordinate clicking, uinput, DOM/CDP, and private
  application APIs remain out of scope.
- Semantic operations remain strictly dependent on public Accessibility
  semantics and operation-specific observation. Raw key delivery can be
  observed without being promoted to semantic success.
