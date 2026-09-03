# GUI2TUI v0.2.0 release-candidate qualification

Qualification date: 2026-09-03

## Result

**GUI2TUI v0.2.0 RELEASE CANDIDATE QUALIFIED**

**READY TO RELEASE v0.2.0 — PUBLIC RELEASE NOT PUBLISHED**

Phase 0.2A, 0.2B and 0.2C remain validated. No feature architecture was
reopened. P0=0 and unaccepted P1=0.

## Source and evidence commits

- RC source commit: `578fcb2dcdfc07954587cc019caff2ba11982659`
- Evidence documentation HEAD: recorded by the commit adding this file
- Public `v0.2.0` tag/release: not created

The RC source commit contains the 0.2.0 version, default spatial presentation,
public documentation and the minimal packaged-smoke compatibility adjustments.

## Compatibility smoke

| Application | Result | Evidence |
| --- | --- | --- |
| Mousepad | PASS | Document-centered scene, Reader, commands, multiline safety |
| Chromium | PASS | Web Document, application Address/Search, context, responsive regions |
| Firefox | PASS WITH SAFE LIMITATION | Reader/search and core surfaces; rejected writes remain read-only |
| EOG / Image Viewer | PASS | Graphical Content, honest modality fallback, compact controls |
| GTK Demo | PASS | Content, GUI tab context, controls and authoritative action read-back |
| Qt Designer | PASS | Multi-region composition, hierarchical navigation, narrow layout |
| LibreOffice Writer | PASS WITH SAFE LIMITATION | Reader/Outline/Search and dialogs; long content is `PartialRealized` |
| VS Code / Electron | PARTIAL | Discovery/search where exposed; anonymous actions and editing remain refused |

These results reuse the existing real-application corpus and AT-SPI-only
workflows. Partial/safe limitations are documented in [compatibility](../../../../docs/compatibility.md)
and [limitations](../../../../docs/limitations.md).

## Semantic/security regression

Existing suites and real workflows continue to pass for password exclusion,
multiline document safety, anonymous-action refusal, stale-generation rejection,
modal confinement and modality boundaries. No application/toolkit-specific
production branch was added.

## Public documentation and assets

README now presents the v0.2 model (semantics + spatial topology → responsive
terminal workflows), documents Region Navigator keys and responsive behavior,
and links real Chromium, Qt Designer, EOG and Mousepad v0.2 captures. The hero
image is a real Qt Designer v0.2 scene; the existing GTK recording remains a
semantic-operation walkthrough and is not represented as the v0.2 hero.

Updated public documents: README, getting started, architecture, spatial layout,
compatibility, limitations, demo walkthrough and
`docs/release-notes-v0.2.0.md`.

## Version and default layout

Canonical Cargo version is `0.2.0`; `gui2tui --version` prints `gui2tui 0.2.0`.
The validated responsive spatial layout is now the normal default. The prior
linear presentation remains available explicitly with `--layout flat`.

## Local quality

macOS arm64: fmt, check, all-target tests (274 library + CLI tests), clippy
`-D warnings`, diff and documentation audit PASS. Local OrbStack Linux had no
Cargo toolchain; Linux quality is independently covered by the successful
Ubuntu 22.04 dual-architecture RC workflow below.

## GitHub RC pipeline

- Workflow: `Release candidate`
- Run ID: `33710697270`
- Source commit: `578fcb2dcdfc07954587cc019caff2ba11982659`
- `publish=false`, attestations enabled
- version gate: PASS
- native x86_64 build: PASS
- native aarch64 build: PASS
- extracted-package smoke (fresh HOME/session): PASS on both architectures
- assembly, exact architecture set, checksums and manifest: PASS
- publish job: SKIPPED

The assembled manifest records exactly these archives:

| Archive | Bytes | SHA-256 | Max GLIBC |
| --- | ---: | --- | --- |
| `gui2tui-0.2.0-linux-x86_64.tar.gz` | 13,417,642 | `5efe93ab8baab8866b9fce6a3dc478c8b33069e049aab54c9d7ac50097563081` | 2.34 |
| `gui2tui-0.2.0-linux-aarch64.tar.gz` | 13,228,484 | `8df9dceb9a4d9c1df30f2b547c1282a170234d688453f1c0b83c8b8f4ef9828b` | 2.34 |

Downloaded RC artifacts were rechecked with `sha256sum -c SHA256SUMS` and
`gh attestation verify` for both archives, `SHA256SUMS` and
`RELEASE-MANIFEST.json`; all returned exit 0. Public-download verification has
not occurred because no GitHub Release exists.

## Remaining issues

- P0: none
- P1: none
- P2: accessibility-dependent labels, partial long-document realization,
  Electron limitations, final theme polish, richer mouse UX and advanced
  command ranking remain documented limitations.

## Release boundary

No `v0.2.0` tag was created and no GitHub Release was published. Explicit user
authorization is required before creating either.
