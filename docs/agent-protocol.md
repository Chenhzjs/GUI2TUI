# Development decision-client boundary

`cargo build --locked --bin gui2tui-agent` builds an opt-in development binary.
It reads one JSON request per stdin line and writes one JSON response per stdout
line. Diagnostics use stderr. The same `TuiApplication`, semantic cache,
interaction scope, runtime generation and operation authority persist for the
connection; the client does not parse terminal rendering.

```sh
gui2tui-agent --session managed --app Firefox
```

Requests:

```json
{"command":"observe"}
{"command":"execute","observation_id":1,"operation_id":"op-12"}
{"command":"execute","observation_id":2,"operation_id":"op-16","parameter":"literal user text"}
{"command":"quit"}
```

Observe returns `ok` and an `observation` containing `version`,
`observation_id`, `session`, `generation`, `interaction_scope`, `partial`,
`nodes` and `operations`. Long operation display labels have a bounded preview
and explicit `label_partial`; canonical node labels, exact action names and
identities are preserved. Invalid parameters are rejected before delivery.
Node IDs and operation IDs belong to that
observation. `partial` preserves tree truncation; a snapshot does not claim
progressive Reader coverage. Password subtrees redact labels, descriptions
and values and expose no agent operations.

For each non-redacted in-scope node advertising public Text, observe reads the
complete public text rather than exporting the terminal's cached preview.
`value_source: "public_text"` and `value_complete: true` identify that read.
A failed read leaves `value: null`, `value_complete: false`, an explicit
`value_error` and `partial: true`; it never silently substitutes the preview.
This does not normalize provider text or expand write capabilities. Exact
operation preconditions still use the normal cache and fresh execution checks.

Operations reuse F3's public capability enumeration. Supported operation kinds
are `action` (exact advertised name), `replace_text` (string parameter),
`selection`, `adjust_value`, and `set_value` (finite number parameter). This
first boundary omits range-editing and text-selection reads. It does not offer
native raw keys or external resource handoff.

Execute consumes the stored observation on every attempt. It expires after
120 seconds and cannot be reused, including after an unknown result. Before
dispatch, GUI2TUI refreshes public Accessibility and checks exact target
identity, role, states, current value, ancestry, actions, capabilities, runtime
session, application generation and interaction scope. Unrelated node changes
do not grant or revoke target authority. The normal backend rechecks the
operation; runtime tickets reject retired publication. Writes use public
backend acceptance and authoritative readback. Whole-text editing preserves
its configured `--max-edit-bytes` budget and complete-text requirements.

A successful execute response contains a `receipt`. Named actions report
`accepted: true` when acceptance is established (`null` otherwise), separately from `effect: "unknown"`: delivery cannot prove the
application's task consequence. Text, selection and value writes report their
verified primitive readback, which is also distinct from overall task
completion. A numeric readback differing from the requested value reports
`effect: "unknown"` and `accepted: null`; the backend mutation result alone
does not establish delivery (for example, an out-of-range adjustment may be a
no-op). An unchanged or normalized value never confirms the requested effect.
Observe again to acquire fresh GUI state before the next decision.

Whole-text readback may additionally contain `text_evidence`. An operation-local
subscriber accepts public TextChanged events only from the exact validated
target after a baseline-reply serial fence. It must establish a complete literal
replacement; its projection using the event spans must exactly match fresh Text
readback with no intervening event. This handles public UTF-16 offset padding
without guessing from an application identity or from the requested value.
`text` then contains the verified literal, while `text_evidence.exposed_text`
preserves the unmodified Text response. Genuine U+FEFF remains literal content.
Missing, malformed, incomplete or inconsistent evidence cannot confirm a padded
readback. Receipts include final literal/exposed values and event spans, but omit
deleted/transient payloads. Observe node values remain the raw public values.

`ok: false` is an error, never an invitation to replay. Invalid protocol or
missing/consumed observation reports `delivery: "not_attempted"`. Rejections before dispatch also report `delivery: "not_attempted"`; the client
may obtain a new observation and ask for a new decision. Other
execution errors conservatively report `delivery: "unknown"`; the client must
not automatically retry. There is no durable cross-process exactly-once claim.
The stream is intended for a trusted local process, not an unauthenticated
network service. It contains non-secret GUI content and must be handled as
private task data.

The separate Jev integration is in `~/Document/Jev-Anything` in the local
workspace, with a generic CLI, explicit task-input candidates and real-browser
acceptance evidence. This bridge is a development source capability, not a
published GUI2TUI release protocol.

## Complete shared document (development, explicitly versioned)

`{"command":"observe_document"}` selects the complete-document observation
path. Its response version is `gui2tui.agent.document.v1`. The existing
`observe` command and `gui2tui.agent.v1` shape remain available. Either command
retires the previous observation. Both use the same `execute` command and
one observation-local execution registry; every execution attempt consumes
that observation, including invalid parameters or a non-invocable ID.

The new response serializes the current shared Rust `TuiDocument`: all legal
public nodes, complete Text payloads, known/unknown/unavailable relations,
content models and navigation, regions, commands, choices, scope boundaries,
source references, public operations and parameter domains. It does not use
terminal dimensions, viewport, Reader page, preview limits, primary-only root
selection or the Human LRU as its source inventory. `text_payloads` holds each
source's read result in this response; content blocks can reference it without
requiring expansion. `nodes.value` retains the semantic cache value separately
and is not a claim of complete Text.

Operation `label` and `label_provenance` serialize the shared core description:
public name, named semantic region with source IDs, or an explicit role fallback.
Human F3 uses this same description and public document ancestors, then adds
its compact presentation and action suffix. It does not obtain labels or hidden
ancestor payload from the Human scene or the unsanitized runtime cache.

Optional research annotations now use `tui.decision_annotations.v2` with
`context_representation: canonical_document_refs.v1`. Context ancestors and
joined endpoints contain `node_ref` IDs into this same response's complete
`nodes`. Public relation reads contain `relation_ref` IDs into `relations` by
source. A boundary-only ancestor with no public relation record retains its
inline `unknown` read. No second request or expansion is needed, and none of
the canonical node/relation payloads is removed. Cohorts, features, unresolved
peers and task-relevance status are unchanged. Research mode remains off by
default. The old frozen v1 annotation records remain valid historical inputs.

The additive `structures_version: "tui.document.structures.v1"` fields
`tables` and `virtual_collections` serialize the same organized structures used
by the Human content reader, excluding its cursor. Every owner, cell, item and
derived `label_source` refers to a public node in this response. Cell positions
state their origin: exact public TableCell evidence, semantic Row child order,
not requested, or unavailable. `table_position_reads` retains the actual public
read outcomes separately from any semantic-order fallback. Missing positions
do not remove cells. Spans remain null when unreadable; partial realized extents
do not establish logical totals. Public positions are addressing evidence, never
object identity or execution authority. Collection `current` and table cursor
`position` are Human state and are absent. Active-descendant acquisition is not
part of this observation vocabulary yet: its status is `not_collected`, rather
than a claim that the application has no active descendant.

`completeness.materialization` distinguishes `complete_for_available_payloads`
from `incomplete_build`; `observed_coverage` is independently
`complete_for_declared_scope`, `partial` or `unknown`. Missing reads, virtual
collections, unavailable fields and unresolved endpoints remain explicit. This
is completeness of the declared public acquisition vocabulary, not all native
interfaces or all future application content. Collection has a time interval
and `atomic_snapshot: false`.

Password subtrees are excluded before organization and serialization. Necessary
Window/Application/scope-owner boundary nodes have `boundary_only: true` and
carry identity/role/structure without labels, values, states or operations.
Scope ownership can differ from AX parentage, such as a sibling modal Dialog.
Outside-scope and redacted relation endpoints are explicit boundaries; unresolved
public references use opaque IDs with a payload status in `boundary_references`.
The wire response contains no raw callable backend objects or locators.

Operation labels have `label_provenance`: the full public name, a source-backed
shared region label, or an explicit role fallback. They do not use scene display
labels. Public Value domains retain current/min/max/step (including read-only
Value facts) or explicit unknown/unavailable status. Native Document metadata
probes retain per-field failures. Duplicate advertised action names remain in
source actions with `name_ambiguous`; no anonymous action is selected.

The shared document can describe existing Human range-edit and text-selection
operations while `agent_invocable: false` records that this development Agent
executor does not implement them. Submitting those IDs is `not_attempted` and
consumes the observation. Supported Agent actions keep the existing exact
identity, session/generation, scope, fresh precondition, ticket, acceptance and
readback chain. The document grants no independent authority. `unknown` is not
permission to replay; `not_attempted` requires a fresh observation and decision.

`timings` separates the AX walk/read groups, arena, capability qualification,
relations, full Text, Value domains, Document probes, shared organization and
Agent Value construction. Overlapping metadata/children request durations are
labelled sums and must not be added into wall latency. The complete observation
request does no Human scene/layout work; CLI startup still initializes the
existing application owner. JSON UTF-8 encoding and write/flush measurements
are emitted to stderr as `agent_jsonl_transport` diagnostics. New-document
receipts additionally report fresh-validation, delivery/readback and total
execute timing; timing does not establish task completion. Provider/model
latency is null when no provider ran. Human scene and render timing use existing
tracing diagnostics. This instrumentation does not claim an optimization.

The AT-SPI walk now schedules at most four independent sibling metadata reads
at a time, after obtaining the parent's public child list. The Agent CLI's
`--ax-read-concurrency 1..32` changes that scheduling bound (default `4`);
it never limits the output inventory. A batch completes before ordered DFS
assembly and identity allocation, so recursive levels do not multiply this
bound. Existing within-node reads and the 32-way complete Text reads retain
their own scheduling. Fresh execution checks, writes and authoritative readback
remain sequential in the existing authority chain.

Walk diagnostics add `metadata_nodes_read`, `metadata_discarded`,
`node_read_batches` and `node_read_peak`. Bounded read-ahead can read a sibling
later excluded by the node budget or a previously visited locator; its metadata
is counted and discarded without publication. Depth-limit `GetChildCount`
reads now also count in `children_requests` and `children_rpc_sum_ms`; older
diagnostics counted only `GetChildren` there. Summed metadata durations can
overlap across nodes as well as within one node. Observation remains non-atomic;
the read schedule grants no new capability or authority.

## Optional shared decision annotations (development research)

Both `gui2tui` and `gui2tui-agent` accept
`--decision-annotations off|relations|contrast` (default `off`). The shared
Rust `TuiDocument` stores one observation-owned result. The Human F3 detail
view consumes its stored hint text; complete-document serialization includes
its full `decision_annotations` object, version `tui.decision_annotations.v1`.
The existing v1 reader remains available; full representation experiments use
`observe_document`, not legacy preview/display labels.

`relations` adds complete per-operation ancestor records and public relation
joins, retaining unknown/unavailable/boundary status. `contrast` adds strict
role/public-label/action/parameter-domain cohorts, source-backed distinguishing
facts and unresolved peers. All public operations participate before client
literal expansion. Repeated advertised action names are marked unresolved and
never selected by an anonymous index.

The first method compares declared content membership, ancestry, public
relations, description, state and value features. It uses greedy source-pair
coverage with a fixed tie order; IDs are references/order for serialization,
not distinguishing semantic values. Unavailable facts are not known empty
sets. Its `task_relevance: not_evaluated` is intentional: a difference, even
one separating every source pair, does not establish the user's target.
Cohort non-membership is not a proof of unambiguous task identity. No nominee,
permission, readback success or model/task completion is produced.

Disabling annotations leaves the original facts and operation catalogue
unchanged. Rebuilding uses the new observation/session/generation/scope;
old annotation provenance cannot authorize an operation. Jev ignores only the
annotation's observation ID, along with other acquisition diagnostics, when
checking whether an unknown operation has changed GUI facts. It retains the
other annotations and all semantic/authority fields.

`timings.decision_annotations_ms` is a sub-interval of
`shared_organization_ms`, including the base wire projection used by this
prototype. Do not add both numbers as independent latency. The method remains
experimental and off by default; local correctness and engineering timing do
not establish a model decision benefit or human efficiency.

## Read-only source references (additive component)

`observe_document` retains `gui2tui.agent.document.v1` and adds the explicitly
versioned `source_identity.version: gui2tui.source_locator_refs.v1` component.
Its namespace is the current runtime session and application generation. Every
public document node has one opaque `locator_ref`, assigned to its exact
BackendLocator in that runtime generation. References survive snapshot node-ID
renumbering and are discarded on application invalidation. No raw locator or
hash of locator, label, body or secret is exposed. References are assigned only
to nodes retained by the existing document privacy/scope checks.

This is read-only provenance for comparing current public observations. It is
not logical data-item lifetime, an atomic snapshot, cross-process observation
sharing, task completion or execution authority. A virtualized/reused address
can refer to changed logical content. The typed serializer verifies the exact
public node set, locator/namespace match and unique references.

Execution continues to require a current observation and advertised operation
ID, its ticket, exact identity/scope/capability checks, backend acceptance and
authoritative readback. Supplying a locator_ref as operation_id grants nothing.
The shared TuiDocument organization and legacy v1 observe command are unchanged.

## Frozen public-projection annotation replay (research)

`cargo run --example replay_annotations -- --mode off|relations|contrast`
reads one complete exported public document on stdin and returns the explicitly
versioned `gui2tui.annotation_replay.v1` wrapper. It invokes the same Rust
organizer as Human/Agent annotations, with structural inventory/reference
checks. The wrapper contains the exact input-byte SHA256 and annotations only;
it does not connect to a GUI, create a runtime, issue a ticket or execute.

This permits paired research inputs with identical source facts, node/operation
IDs, scopes, payloads, parameter domains and observation provenance. A thin
research driver may attach the returned annotation to a copy of that exact
source document. Producer authenticity and privacy still come from the original
G2T export; structural replay validation is not producer authentication or new
execution authority. Old observations/renamed research IDs never become fresh
operations. The underlying feature/cohort/greedy rules and default off policy
are unchanged. Replay output is not task completion or a method-benefit claim.


### Selection state already satisfied (additive result v1)

Selection is a requested state, not an instruction to force another mutation.
After normal identity/scope/ticket checks, the backend can establish exact current
child membership and confirm `Selection.IsChildSelected` already equals the
requested boolean. It then sends no SelectChild/DeselectChild write. The
observation is still consumed. This is not an atomic-state guarantee or task
completion evidence.

The explicit response has `accepted: null`, `delivery: "not_attempted"`, and
`effect: "already_satisfied"`. Its readback includes
`selection_outcome_version: "gui2tui.selection-outcome.v1"`, the requested
`selected` boolean, `already_satisfied: true`, and `write_invoked: false`.
It must not be reported as an accepted backend write. A normal accepted write
retains `effect: "confirmed"` and has `write_invoked: true` in this versioned
Selection readback. Actual rejections/timeouts are not converted into success.

A compatible client validates this no-write result against the current
operation/observation and requested boolean, observes again, and makes a new
decision. It does not infer task completion or replay an uncertain write. Older
clients that stop on null acceptance continue to stop conservatively.


## Public named-action explanations

Complete document v1 operations may additionally carry
`action_semantics.version = "gui2tui.action-semantics.v1"`. This optional,
backward-compatible explanation comes from the same Rust helper used by Human
F3 results/details. It preserves the exact operation `name`, ID, target,
parameter domain and invocation capability.

`effect_semantics: "provider_defined"` describes the action contract, not the
result of an execution. Provider descriptions/keybindings are explicitly marked
as public source metadata. They do not authorize key delivery. The G2T display
label and note are grounded in public role/action metadata and confer no new
capability. In particular, `activate` on a text input is shown as `Input action:
activate`, with a note that submission/navigation is not established by that
name. G2T does not reinterpret it as Focus, Submit, Enter or a different button.

The experimental TUI export includes the same descriptor in its current visible
operation references, so a thin reader retains it without reproducing the rule.
Backend acceptance, verified effect, public state change and task completion
remain separate. Legacy `observe` v1 operation shape is unchanged.

## Experimental shared Human Reader navigation

`observe_tui_text` renders the actual Human spatial TUI with explicit
`gui2tui.agent.tui-text.experimental.v3` metadata. This is partial viewport
content (`complete_document: false`); the complete current document remains in
the same local response for validation. It is not a lossless model-input mode.
`tui_navigate` publishes `gui2tui.agent.tui-navigation.experimental.v2` receipts.
Legacy `observe`, document v1 and their execution registry remain unchanged.

The advertised `navigation_tools.actions` are mode-dependent. `open_reader`
uses the existing Rust content catalogue, optionally matched by literal
`query` title and public ancestor `context`. Without filters, the current
readable focused root can be used. Multiple matching roots remain ambiguous;
the inventory is not restricted to a primary document. `scroll_down/up` moves
ten wrapped terminal lines in Reader mode, including within one large AX text
block; `next_control/previous_control` moves one line. Human Reader keys use the
same command handler and renderer. `open_outline`, `find_text` and
`choose_content` browse the current content model's headings/search results;
`close_reader` returns to the scene. Tables and virtual collections retain
their existing Human behavior; this extension qualifies text Reader, outline
and search navigation only.

Reader metadata reports root, mode, position, wrapped-line offset, query,
result count/selection, heading count and coverage. The Reader overlay exports
no covered base-scene `visible_bindings` or focused GUI target. Reading a link
or a search result does not invoke it. Explicit control/action search can leave
the Reader, after which only current visible operation references can be used.
All local navigation receipts have `gui_write: false` and
`activation: "not_attempted"`; they confer no operation authority.

Each fresh complete observation reconciles an open Reader against current
scope, source existence and secret exclusion, closes an invalid source and
refreshes available payload. Fresh loaded or unavailable complete-document
text supersedes an older Human range cache even without an AX event. Unknown
coverage remains unknown. Terminal scrolling, selection and folding do not
alter the complete document's facts or execution permissions.

Execution precondition rejection now identifies changed field categories such
as `node.states` or `ancestors` without including source values. The equality
check is unchanged. The client must still observe and decide again after a
`not_attempted` result; an `unknown` result never permits automatic replay.


### Versioned current-publication batch execution

`execute_batch_v1` is an additive experimental command requiring `--spatial-tui`:

```json
{"command":"execute_batch_v1","observation_id":1,"operations":[{"operation_id":"op-1","parameter":"first literal"},{"operation_id":"op-2","parameter":"second literal"}],"width":120,"height":40}
```

The entire1..4 distinct-operation list is validated against the original
unconsumed publication and existing parameter validators before any GUI write.
A rejection consumes the original public observation just like ordinary execute.
Only that frozen candidate list is authorized; it never acquires new targets by
name, ID continuity, order, geometry or locator rebinding. Each original candidate
uses ordinary `agent_execute` fresh preconditions, exact authority and public
backend/readback validation. The publication expiry is not reset per step.

After each attempt G2T captures fresh public state. Changed preconditions,
modal/scope exclusion, rejected delivery, unavailable acceptance or unknown
primitive effect stop the remaining list. Earlier delivered steps are not rolled
back; this is not an atomic transaction. The response includes
`gui2tui.agent.batch.v1`, ordered attempted `items`, `status`, `reason`, and the
latest `observation` / `tui_view`. A post-operation observation failure yields
unknown with no executable current publication. The returned fresh observation,
when available, may be used for a new decision; the consumed original cannot.

Exact `find_action` / `find_command` queries also match a current public node
name independently of role/path decoration. Names are read only for the exact
current locator in the legal non-secret scope. Existing full label/title and
context matching remains; multiple matching operations still report ambiguity.


`open_actions` and `open_commands` also honor a supplied nonempty query by calling
the same literal palette finder (including exact/context/ambiguity). Without a
query they preserve ordinary whole-palette opening. This remains local navigation,
with `gui_write:false` and `activation:not_attempted`; it never dispatches a GUI
operation or limits the complete underlying document inventory.
