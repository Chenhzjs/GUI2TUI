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
