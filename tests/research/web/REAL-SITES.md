# Live production website validation

`real_sites_v2.json` complements the local workbench and historical public-site
batch. The source is the live HTTPS page opened in a fresh real browser, with no
local fixture server, downloaded replacement page, mirror, DOM/CDP driver or
screenshot semantics. All site names and selectors are validation data only.

The two groups of evidence are:

- One continuous Wikipedia Accessibility article session: complete introductory
  text, headings, image alternative description, table and list ancestry,
  contents expansion/collapse, text size, page width, colour preference, literal
  English/Chinese search editing and clearing, and an article fragment link.
- Five fresh sessions, one task per site: Python Tutorial navigation, Rust Book
  chapter navigation, MDN sidebar input editing, PyPI package search, and GitHub
  README file navigation. PyPI search can expose real service/bot restrictions;
  those are recorded rather than replaced with a demonstration page.

Each task has a natural-language goal, supplied literal parameters, initial
state where needed, maximum six steps, exact public operation selectors for
direct mode, and conjunctive independent read-only AT-SPI assertions. Document
URLs are read through public Document attributes. Assertions are scoped to the
expected exact document, so browser chrome and background documents cannot
satisfy them. The complete-paragraph assertion also compares the G2T value with
the independent full Text response; a preview must not pass as full content.
Read-only checks also require the corresponding content in the G2T observation.
The initial v1 manifest is retained separately: its table-header role was too
coarse and was corrected from actual public column-header evidence in v2.

```sh
~/Document/Jev-Anything/.venv/bin/python tests/research/web/real_sites.py \
  --binary /absolute/path/to/gui2tui-agent \
  --image sha256:EXACT_IMAGE_ID \
  --output /absolute/path/to/new-run
```

`--groups wikipedia-multi` selects just the continuous article session. Every
invocation requires a new output directory. The manifest, source files and
hashes, binary hash, image ID, initial and subsequent independent observations,
actual G2T requests/responses, selected operations, receipts, final snapshots
and per-task results are preserved. A preliminary failed session remains a
separate attempt; reruns never overwrite it or the historical frozen package.

Direct mode also exercises Jev's actual full candidate/context builder through
an offline recorder. Its answer is never executed, and no provider is called.
The complete `state` and `questions` are saved for reconstructing model inputs.
`--mode jev` uses the existing production Jev loop, full candidates and the same
independent oracle, logging actual decisions, provider request metadata/usage,
wait, abstain, done, unknown and step-limit results. The default direct result
does not establish model decision quality. Provider limits must be resolved at
deployment level without silently deleting public nodes or candidates.

Acceptance, primitive effect and independently asserted task completion remain
separate fields. Unknown delivery is not replayed. Each direct plan is freshly
resolved before dispatch; ambiguity is an error. Failed task preconditions do
not become passes just because the requested final state was already present.

These tasks read public content, navigate and change anonymous browser-local
preferences/search fields. They do not sign in, edit repository/article data,
post messages, submit transactions or change application backing files.
