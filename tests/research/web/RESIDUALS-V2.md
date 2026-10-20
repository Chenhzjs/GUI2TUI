# Separate revision of the residual direct plans

`deep_tasks.json`, `run.py`, the fixture and the frozen archive remain unchanged.
`revised.py` applies `residual-plans-v2.json` as an explicit, opt-in validation
revision. Its result must never replace or be combined with the frozen score.
Production G2T and Jev code do not import these selectors or fixture labels.

The natural-language goals, literal parameters, order and initial page remain
the same. Three direct plans change:

- `delivery-express` uses the showing popup `MenuItem`'s exact public `click`.
  The underlying option's public `select` was accepted without establishing
  either the selected state or the page's delivery event. A Selection membership
  mutation is a different primitive and does not promise a page `change` event.
- `delivery-membership` identifies the exact `MenuItem` under the `Delivery speed`
  `ComboBox`. A label alone also matches a separate popup item. The oracle now
  requires a unique matching public menu item and its selected state.
- `advanced-close` requires an Expanded target and invokes the exact advertised
  `expand` action. Separate real-GUI evidence confirms this action collapses an
  expanded target. This is a fixture validation plan, not a production rule
  translating every `expand` into a collapse operation.

`name-unicode` keeps its original literal text and strict public Text oracle.
It additionally enforces the goal's “do not submit” constraint. Public AT-SPI
Text currently appends U+FEFF after the non-BMP scalar in this environment.
Raw D-Bus reads reproduce it before G2T interpretation. A separate local-fixture
submit witness verifies the literal application value, but is not counted as a
passing no-submit task. Genuine U+FEFF is also preserved by public readback, so
silently removing it would corrupt a different requested literal.

Run only the revised direct batch (no provider calls):

```sh
~/Document/Jev-Anything/.venv/bin/python tests/research/web/revised.py \
  --binary /absolute/path/to/gui2tui-agent \
  --image sha256:IMAGE_ID \
  --output /absolute/path/to/a-new-run-directory
python3 -m unittest discover -s tests/research/web -p test_revised.py -v
```

The runner saves the effective manifest, source/input identities, actual bridge
requests, receipts, readback, independent per-task oracles and summary. It fails
on ambiguous selectors and does not choose the first match. Extra readback is
read-only; unknown writes are never automatically replayed. It remains a direct
batch and makes no claim about model selection.

Other frozen-oracle limitations remain visible rather than being silently
repaired here. In particular `delivery-open` only tests the presence of an option
that also exists while closed. Several tasks omit checks for forbidden side
effects, and an already selected item may satisfy a membership oracle. The
frozen `gui_writes` field counts successful execute responses, not all attempted
or actual mutations; zero does not prove that nothing was delivered.
