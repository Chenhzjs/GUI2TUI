# v1.1 architecture convergence analysis

## Current authority path

The current path is now:

```text
SceneBinding
  -> UiIntent / SemanticOperation
  -> OperationAuthority
  -> RuntimeSession + ApplicationGenerationId + InteractionScope
  -> OperationTicket
  -> fresh BackendLocator / action or capability resolution
  -> AT-SPI delivery or NativeInputDelivery
  -> SurfaceSnapshot / bounded Accessibility observation
```

`src/tui/app.rs` owns operation orchestration and publication. It does not
own an independent native-input authority. `src/tui/actuation.rs` owns only
window resolution, X11 activation, exact Accessibility focus verification,
low-level keyboard delivery, and verified text delivery. The final native
input gate checks the captured operation ticket as well as the authority
snapshot.

## Semantic Submit versus Raw Enter

`SemanticOperation::SubmitNode` resolves fresh compatible advertised public
actions and invokes them by current action identity/name. The saved numeric
action index is not used for later invocation. If no current semantic evidence
exists, Submit is unsupported. If delivery is accepted but the bounded
observation has no relevant change, the result remains a delivered operation
with `NoDetectableChange`; it does not trigger native Enter.

`SemanticOperation::SendKeyEnter` is separate. It is created only by the
explicit `UiIntent::SendKeyEnter` path (the current TUI shortcut is
Alt-Enter). It resolves a current exact target, owning native window, and
exact Accessibility focus, then emits the one currently exposed raw key.
Its trace reports key delivery and an observed consequence; it does not
rename the operation to Submit.

## Edit path

`commit_edit` first resolves `SemanticOperation::ReplaceText` through the
normal scene operation resolver. `NativeInputDelivery::edit` then selects
source, edit, focus, and readback targets independently. Accessibility text
mutation is attempted first and every attempt is followed by fresh
authoritative readback. Only a strict match yields `EditVerified`. The bounded
X11 native text fallback is ASCII-only and is also followed by strict
readback; it cannot silently authorize Submit.

## Surface/content boundary

`RegionAnalysis` retains structural regions and collapses only implementation
wrappers. Multi-child wrappers containing structural regions are retained;
multi-child wrappers made only of direct controls/fields/content may collapse
without losing bindings. `compress_content_scene` preserves the normalized
scene and adds a Reader entry point rather than replacing it. The ordinary
Surface preview is explicitly bounded; Reader owns continuous materialization,
outline, search, and paging.

## Static audit

- XTEST references are confined to `src/tui/actuation.rs`, the low-level X11
  backend.
- Production semantic code contains no application, browser, title, or
  process-name routing branch. Validation names remain in validation fixtures
  and launcher/test catalogs.
- No mouse injection, coordinate targeting, uinput, DOM/CDP, or arbitrary key
  sequence API was added.
