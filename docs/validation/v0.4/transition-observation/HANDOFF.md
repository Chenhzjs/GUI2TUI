# GUI2TUI v0.4 Phase 0.4A — Exact Authority and Bounded Transition Observation

## Status and scope

Phase 0.4A is validated. Production commit `f6a1766` adds one short-lived,
operation-adjacent observation contract for already-supported semantic
actions. It adds no interaction family, user key, menu framework, dialog
subsystem, Expand affordance, Selection, file chooser, automatic next step,
workflow engine, DSL, transaction layer, or task runner.

The phase answer is **YES**: a dynamic operation can be classified using exact
pre-operation authority, a small explicit postcondition, a deadline, event
wakeups, and fresh authoritative GUI reads. Invocation return, event arrival,
and elapsed time do not establish semantic success.

## Implemented architecture

`OperationAuthority` captures the current `RuntimeSessionId`,
`ApplicationGenerationId`, selected application locator, target
`RuntimeNodeId`, exact target `BackendLocator`, `InteractionScopeId`, and exact
scope locator immediately before invocation. The current cache and scope are
validated again before the public action. The backend then re-resolves the
exact advertised action name at invocation; it never guesses an index.

`TransitionObservation` is local to that operation and contains captured
authority, one internal semantic condition, the absolute deadline, and the
existing operation-ticket cancellation token. The ticket remains responsible
only for session/generation ownership and cancellation; it did not become a
target, condition, or workflow registry. Pending observation state is dropped
at confirmation, rejection, stale/cancelled/application loss, ambiguity, or
timeout.

The implemented conditions are deliberately closed and small:

- exact target state present/absent, used for checked and Showing state;
- a new authoritative active `ModalDialog` scope relative to the prior scope;
- exact prior temporary scope absent and inactive, used for modal exit.

State conditions refresh the exact node where that is complete enough. Menu
Showing and scope conditions rebuild the application through the existing
cache/scope/scene pipeline because their realization is not confined to one
node. There is no string predicate, query DSL, name matcher, relation guess,
or arbitrary callback condition.

Outcomes are `Confirmed`, `Timeout`, `Stale`, `ApplicationGone`, `Cancelled`,
`Ambiguous`, and `Unverifiable`; backend rejection continues through the
existing backend error path. An accepted action whose explicit condition is
not reached remains unconfirmed. A deadline performs a final authoritative
read and can only terminate observation, never create success.

## Presentation identity versus operation authority

The controlled cache regression replaces checkbox locator `/old` with `/new`
under the same application and unique structural fingerprint. Existing cache
reconciliation deliberately retains the same `RuntimeNodeId`. Authority
captured for `/old` then returns `Stale`; it cannot invoke or confirm `/new`.
A new authority capture from the rebuilt current binding for `/new` succeeds.

This establishes:

```text
presentation RuntimeNodeId continuity
  does not imply
operation authority transfer across BackendLocator replacement
```

The live Qt fixture independently replaced the Username control while an edit
was open. The old AT-SPI locator became unavailable and was refused. After the
user cancelled the old local edit, refreshed normally, and selected the new
current binding, a new write to the replacement was authoritatively confirmed.
No name, geometry, child index, or fingerprint granted the old operation
authority over the replacement.

## Event and deadline model

Every observation performs an immediate fresh semantic evaluation where
useful, then re-evaluates after either invocation completion or an event
wakeup. Event bursts use the existing 40 ms batch window and bounded event
subscription; overflow uses the existing one-shot resynchronization path.
Before an outcome is evaluated, the observer makes a fresh backend read at the
condition's safe refresh boundary. Event-applied cache data alone cannot
confirm a condition.

The final evaluation at the deadline is also authoritative. There is no busy
polling and no sleep-based success. Validation loops use bounded deadlines
only to terminate the probe while waiting for product evidence; they do not
establish product correctness.

An event need not be causally attributed to the action. An unrelated event may
wake observation, but a false postcondition remains false. Conversely,
invocation completion may wake a fresh read and confirm a true postcondition
when no useful transition event reaches the observer.

## Live Linux evidence

Validation ran on 2026-09-05 in the established `gui2tui-live` Ubuntu 24.04
arm64 environment with Xvfb/X11, session D-Bus, AT-SPI, the existing Qt6 and
GTK4 fixtures, and GTK Demo. The runner is
`tests/live/v04a_run_linux.sh`; its bounded TUI/Inspector probe is
`tests/live/v04a_transition_observation.py`.

### Menu

The existing Qt Tools control exposed exact `ShowMenu`. Before invocation its
known Menu existed but was not Showing. The normal TUI invoked the exact named
action, event wakeups caused refresh, and a fresh full semantic read confirmed
the exact Menu locator as Showing/Visible. The refreshed tree exposed
`Activate Demo`; an exact public Press changed the fresh application status to
`menu activated`.

The popup still exposed no public owner relation. Showing confirmation did not
grant it a new `InteractionScope` or infer ownership. No production menu
navigation or framework was added.

### Disclosure

GTK Demo `Constraints` exposed the exact named `listitem.collapse` and
`listitem.expand` actions plus Expanded state. A bounded fresh read confirmed
Expanded absent and realized rows absent after collapse. The exact expand
action restored Expanded and the rows; fresh state verified restoration.
Expanded and children event families were useful wakeup evidence, not proof.

The rows remain siblings without an exclusive public owner relation to the
trigger. 0.4A therefore confirms trigger state and scene realization only; it
does not claim ownership or add production Expand support.

### Modal enter and exit

The Qt `Open modal dialog` Press began in the authoritative main Window scope.
The public D-Bus call can remain blocked while `dialog.exec()` is active, so
invocation and observation run concurrently. A fresh full read established a
new active `ModalDialog`; the existing `InteractionScope` analysis hid blocked
background authority and rebuilt the normal scene for manual user
continuation. The bounded client wait was then retired without dismissing the
GUI-owned dialog.

Exact Close from the current modal scope used the prior exact scope locator as
its condition. A fresh full read confirmed that scope absent/inactive and the
main Window restored as active. No dialog-specific state machine was added.

### Missing event, unrelated event, and timeout

The Qt checkbox Toggle completed through the invocation wakeup followed by a
fresh exact-node read. Checked became authoritative with
`event_wakeups=0`; no useful matching event was required.

The Qt `Activate safely` action changed a status label and emitted unrelated
semantic traffic while the explicit `new active modal` condition remained
false. The observer woke and re-read but never confirmed. Its final fresh read
at 500 ms remained false, so the result was `Timeout`; the changed status was
visible in fresh GUI semantics but did not satisfy the wrong condition.

### Event burst

The existing GTK fixture generated 2,000 accessibility mutations while the
event buffer was intentionally bounded at 128. Existing batching/overflow
resynchronization reduced this to one observer wakeup in the recorded run. A
fresh read showed `Storm complete: 2000`; the unrelated modal condition timed
out without false success, and the TUI remained usable. No new scheduler,
event registry, history, or backpressure subsystem was introduced.

## Required live result summary

```text
TRANSITION_MENU_STATE_CONFIRMATION=PASS
TRANSITION_DISCLOSURE_STATE_CONFIRMATION=PASS
TRANSITION_MODAL_ENTER_CONFIRMATION=PASS
TRANSITION_MODAL_EXIT_CONFIRMATION=PASS
TRANSITION_UNRELATED_EVENT_REFUSAL=PASS
TRANSITION_NO_EVENT_AUTHORITATIVE_CONFIRMATION=PASS
TRANSITION_TIMEOUT_NO_FALSE_SUCCESS=PASS
TRANSITION_STALE_LOCATOR_AUTHORITY_REFUSAL=PASS
PRESENTATION_ID_AUTHORITY_SEPARATION=PASS
TRANSITION_EVENT_BURST_WAKEUPS=1
TRANSITION_EVENT_BURST_COALESCING=PASS
```

## Cancellation and application loss

Transition observation uses the current operation ticket's
`CancellationToken`. Session invalidation, generation replacement, or
application loss cancels current tickets. Observation validates the original
session/generation/application before every read; it never migrates to a
replacement generation. Event-stream reconnect is followed by classification
against the resulting current runtime, producing stale/application-gone rather
than continuing under new authority.

An expected post-invocation scope disappearance is not classified as stale:
modal exit explicitly checks that the exact old scope is absent and no longer
active. The initiating control need not remain available for that condition.

## Automated regression and phase-close quality

Three focused tests cover stable invariants:

1. preserved `RuntimeNodeId` plus changed `BackendLocator` never transfers old
   authority, while a fresh current binding may capture new authority;
2. an unrelated state change cannot satisfy an exact state condition;
3. fresh state can confirm without an event, while a timeout report never
   becomes success.

Phase-close macOS results:

- `cargo fmt --all -- --check`: pass;
- `cargo check --all-targets`: pass;
- `cargo test --all-targets`: pass — 281 library, 2 Inspector CLI, and 4 user
  CLI tests;
- `cargo clippy --all-targets -- -D warnings`: pass.

Linux built current binaries, passed all live workflows above, and passed the
three focused transition tests. Shell syntax, Python probe compilation,
`git diff --check`, and the documentation/link audit passed at phase close.

## Existing capability and genericity audit

Single-line Text, Value, ExternalTextSession, conflict handling, password
exclusion, Choice, Reader, content, spatial layout, and Region Navigator were
not refactored. Choice still reconstructs complete options directly without
reproducing a GUI popup lifecycle. The normal cache, scope, scene, and focus
machinery remains the only presentation path after a transition.

Production uses only role, state, exact public action name, exact locator,
session/generation, and semantic scope. Application/toolkit names occur only
in fixtures and evidence. There is no input injection, coordinate operation,
fixed-delay success, anonymous action, fuzzy identity, backing-file bypass,
private toolkit API, or app-specific branch.

P0: 0. P1: 0. P2: 0.

## Conclusion and next recommendation

**PHASE 0.4A EXACT AUTHORITY AND BOUNDED TRANSITION OBSERVATION VALIDATED**

The evidence confirms that the narrower operation-adjacent contract is
sufficient. It did not require a `WorkflowEngine`, workflow DSL,
`CompoundSemanticOperation`, general task runner, persistent Task object, or
automatic continuation.

Recommend **0.4B — Dynamic Surface and Scope Continuation** for user review.
0.4B is not automatically authorized. v0.5 task/interaction completeness,
v0.6 runtime continuity, and v0.7 deployment completeness remain future
roadmap layers.
