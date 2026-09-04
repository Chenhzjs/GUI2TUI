# GUI2TUI v0.3 Phase 0.3B Native Value Validation

## Status and scope

Phase 0.3B is validated for its explicitly authorized scope: Value only. It
adds terminal-native adjustment for qualified bounded AT-SPI Value controls.
It does not implement Selection, multiline mutation, external handlers,
Expand/Collapse, compound operations, or release work.

Production commit: `611c99e` (`feat: add verified native value interaction`).

## Semantic eligibility

`SemanticCapability::Value` is assigned only when all of the following live,
public evidence is available:

- the original AT-SPI role is Slider or SpinButton;
- the Value interface is exposed;
- Enabled is present and ReadOnly is absent;
- CurrentValue, MinimumValue, MaximumValue, and MinimumIncrement are readable
  and finite;
- minimum is not greater than maximum, current is within the range, and the
  advertised increment is positive.

The original AT-SPI role is retained for this decision because GUI2TUI's
`SemanticRole::Slider` also represents ScrollBar. ScrollBar, ProgressBar,
LevelBar, Dial, missing/invalid metadata, zero/negative increments, and
read-only controls do not receive the capability. These are generic semantic
rules; no application or toolkit identity is consulted.

## Operation path

The implemented path is:

```text
focused SceneElementKind::Value + Up/Down
→ UiIntent::IncreaseValue / DecreaseValue
→ SemanticOperation::AdjustValue
→ SceneBinding capability and locator resolution
→ BackendOperation::AdjustValue
→ AtspiBackend::adjust_value
→ Value.set_current_value(requested)
→ independent Value.current_value()
→ refresh_node + SemanticCache refresh
→ rebuilt TuiScene with authoritative value
```

The backend rechecks role, Value interface, Enabled/ReadOnly state, current,
bounds, and increment immediately before mutation. An out-of-range next step
is not sent. NaN, infinity, invalid bounds, and invalid increments never reach
the setter. No arbitrary step or display unit is invented.

## Authoritative outcomes

Setter return is only invocation evidence. `AtspiBackend::adjust_value`
independently reads CurrentValue after every setter call and returns the
previous, requested, and resulting values. The TUI classifies and presents:

- confirmed when resulting equals requested and differs from previous;
- confirmed-normalized when a finite in-range result differs from both;
- unchanged when authoritative result still equals previous;
- rejected when the public call rejects/fails;
- stale/unavailable when the object, interface, state, or metadata no longer
  qualifies;
- unverified when timeout/D-Bus failure prevents authoritative confirmation.

Only the fresh resulting value enters the refreshed scene. Timeout or
unverified failure triggers an authoritative refresh and never installs the
requested value locally. Technical D-Bus details remain available to debug
logging; normal status text stays semantic.

## Terminal presentation

The compact presentation is numeric and terminal-native:

```text
> Probe value: 4  [↑ increase / ↓ decrease]
> Probe value: 5  [↑ increase / ↓ decrease]
```

Up applies one advertised increment and Down subtracts one advertised
increment only while a qualified Value element is focused. Elsewhere these
keys retain their existing scrolling behavior. Enter does not silently adjust
the value, mouse selection only focuses it, and the command palette does not
manufacture a Value command.

## Live Linux evidence

Validated on 2026-09-04 in the existing `gui2tui-live` OrbStack Ubuntu 24.04
arm64 environment with Xvfb/X11, session D-Bus, AT-SPI 2.52, and Qt6. The
existing Qt fixture was temporarily extended in the working tree with a
validation-only Slider and ProgressBar, then restored; no fixture change was
committed.

Controlled Slider evidence:

```text
role=Slider name="Probe value" current=4 min=0 max=10 increment=1
TUI initial:  Probe value: 4  [↑ increase / ↓ decrease]
TUI Up:       setter issued; fresh CurrentValue=5; scene displayed 5
TUI Down:     setter issued; fresh CurrentValue=4; scene displayed 4
result:       TUI_VALUE_END_TO_END=PASS; original state restored
```

The adjacent ProgressBar exposed numeric value 4 but the inspector marked it
`[read-only]`; it was not rendered with increase/decrease interaction.

EOG 45.3 was inspected independently with an existing project image. Its tree
contained six semantic Slider nodes originating from navigation scrollbars;
none became a writable Value scene control and the normal TUI showed zero
increase/decrease rows (`EOG_SCROLLBAR_NOISE=PASS`). No EOG state was mutated.

## Safety and genericity audit

- Active `InteractionScope` filtering removes blocked background bindings;
  Value adds no scope mechanism of its own.
- Current scene/runtime identity and exact `BackendLocator` are rechecked;
  disappeared/replaced objects fail stale rather than being reconciled
  fuzzily.
- Password handling and redaction are unchanged; PasswordText cannot satisfy
  the eligible role gate.
- Progress/status Values and scrollbars remain non-writable.
- No application/toolkit branch, private API, action-index guess, anonymous
  action fallback, keyboard/mouse injection, coordinate operation, backing
  file mutation, or optimistic cache write was added.

## Tests and quality evidence

Two focused stable-invariant tests were added:

- Value capability requires a qualified adjustable role and live metadata;
- a qualified Value resolves through the existing semantic operation pipeline
  without relying on an AT-SPI Action.

The existing library suite passed with 276 tests. Phase-close platform and
lint results are recorded in the final handoff after the documentation commit.

## Conclusion and next recommendation

Value confirms the v0.3 model across a second mutation family: public
interface exposure becomes user capability only after generic semantic
eligibility, bounded invocation, and authoritative application read-back.
Correct non-promotion of progress and scrollbar Values is part of the result.

The evidence supports recommending Phase 0.3C, Configurable Complex Text
Interaction, next. It remains awaiting explicit user authorization. Selection
is still partially qualified and not implemented; Phase 0.3D and release work
are not authorized.
