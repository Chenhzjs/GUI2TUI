# GUI2TUI v0.3.0 release-candidate qualification

## Status and source identity

**GUI2TUI v0.3.0 RELEASE CANDIDATE QUALIFIED**

**READY TO RELEASE v0.3.0**, subject to separate production-release
authorization.

- Starting HEAD: `74d2995db6718ac757cdd6c3dc79ebb6b043eaca`.
- Branch: `v0.3/capability-recovery`.
- RC source commit: `efc704adf8a3ded3463ed8bb81670eddd08296c3`.
- RC evidence HEAD: the documentation-only commit containing this record. It
  follows the RC source and is not release source; resolve its exact identity
  with `git rev-parse HEAD` in the checked-out handoff state.
- Candidate version: `0.3.0` in `Cargo.toml`, `Cargo.lock`, packaged metadata,
  manifest, and extracted `gui2tui --version` output.
- P0: 0. P1: 0.
- Public release performed: no.
- `v0.3.0` tag created: no.

The future `v0.3.0` tag must point exactly to
`efc704adf8a3ded3463ed8bb81670eddd08296c3` if this candidate is released
unchanged. The later evidence-only HEAD must not be used as the release source.

## Source preparation and freeze

The source candidate includes the final version metadata, v0.3 README and
release notes, getting-started and configuration guidance, compatibility and
limitation corrections, existing demo references, and the existing release
pipeline's package-level v0.3 smoke. Production Rust under `src/` is unchanged
from the completed functional-development head.

RC-relevant source commits are:

1. `4e0907263f03f53ebd8a3a4ab418ca8ada45c5a4` — prepare the v0.3.0
   candidate. Superseded after Ubuntu 22.04 showed that `python3-pyqt6` was not
   available.
2. `17b560e9d42dd2fd797e36fc4a71c3083b61919e` — use an available Qt binding
   for the release Value fixture. Superseded after the Jammy GTK4 fixture
   correctly failed the complete-text qualification gate.
3. `d1869b2f38866fb6dca796f6d41057b651859047` — attempt Mousepad as the
   packaged complex-text target. Superseded because Mousepad exited before
   qualification on the hosted Jammy runner.
4. `efc704adf8a3ded3463ed8bb81670eddd08296c3` — use a controlled GTK3
   complete-text fixture with an inert synthetic backing file. This is the
   final frozen RC source.

The final fixture is release-validation infrastructure, not a product branch:
it supplies a complete non-secret text target, while the extracted packaged
binary still performs eligibility, conflict checks, public AT-SPI write-back,
and authoritative read-back through normal production code. The fixture loads
initial text from a synthetic file but never saves it, so the smoke also proves
that GUI2TUI did not bypass GUI authority by editing backing bytes.

## Public release story and documentation

v0.3 is **Verified Capability Recovery**: public interface exposure and a
successful backend invocation are insufficient by themselves. GUI2TUI exposes
mutation only after generic eligibility, current identity/scope, safe bounded
invocation, and independent authoritative GUI read-back.

The release source documents:

- native verified single-line text through the existing `EditSession`;
- finite, bounded and explicitly adjustable Value controls using authoritative
  `CurrentValue` read-back;
- an optional shell-free `program + args + {file}` complex plain-text handler;
- GUI2TUI-owned private candidate artifacts, pre-write conflict checks, public
  AT-SPI write-back, and full authoritative text read-back;
- safe refusal for passwords, incomplete/rich text, quarantined Qt multiline
  Text, anonymous actions, informational Values and unconfigured handlers.

The exact configuration syntax is:

```toml
[interaction.complex_text]
program = "custom-editor-command"
args = ["--wait", "{file}"]
```

It is direct argv execution, not shell evaluation. No editor is required for
normal startup. Existing demonstration evidence records Vim working only as a
generic configured handler; production contains no Vim special case and does
not claim universal editor compatibility.

README, getting-started, architecture, compatibility, limitations, project
guide, roadmap, `docs/release-notes-v0.3.0.md`, and demo references were
audited before freeze. `AGENTS.md` was unchanged.

## Documentation, privacy and media audit

On the frozen source, `python3 scripts/check-docs.py` passed with 56 files and
177 checked local links. After adding this evidence record it passed again with
57 files and 178 links. Release examples contain no developer checkout path,
credential, password, private hostname, recovery artifact, or personal
document.

Existing v0.3 media decoded successfully and was not regenerated:

| Asset | Evidence |
| --- | --- |
| `docs/demo/v0.3/hero-v0.3.mp4` | 32 s; 279,617 bytes; SHA-256 `0f2209aa6085afb8075e294b7c7f49478822de9859fa0533acbcd47641e3b8d7` |
| `docs/demo/v0.3/demo-v0.3.mp4` | 62 s; 636,284 bytes; SHA-256 `e0137003fa4986dc55de5fecea9a74cb4978cc0d506108fb6a89d242663c8b5e` |
| `docs/demo/v0.3/value.png` | 1440x900; SHA-256 `d2bb85c95d77e6254501014115ce0c2d9514674a08cf529cda4830d89ebfbf3c` |
| `docs/demo/v0.3/external-edit.png` | 1440x900; SHA-256 `b7337488b48aa6d549945eb9d25421218918ade9e8b6aff5eaa189bc9671d3f5` |
| `docs/demo/v0.3/conflict.png` | 1440x900; SHA-256 `10df1ef4509857e67ba5badac7ed04d22c0c10f009f75460b1c215280720a544` |
| `docs/demo/v0.3/safe-readonly.png` | 1440x900; SHA-256 `b0d47787c82a4a0c7b88b971708a3269e60d9f63b37360b507285a173dd9cc7b` |

Visual inspection found only synthetic content and no private data. README uses
the real assets and does not present v0.3 as universal GUI editing.

## Final quality matrix

The normal final quality matrix ran once on the exact frozen source with Rust
1.88.0 and passed:

```text
cargo fmt --all -- --check                         PASS
cargo check --locked --all-targets                 PASS
cargo test --locked --all-targets                  PASS (284 Rust tests)
cargo clippy --locked --all-targets -- -D warnings PASS
python3 scripts/check-docs.py                      PASS
release assembly unit tests                       PASS (6 tests)
release Python compile and shell syntax            PASS
git diff --check                                   PASS
```

The GitHub release jobs also used Rust 1.88.0 on native Ubuntu 22.04 x86_64
and aarch64 runners. No release-level spatial benchmark campaign or new broad
test framework was added.

## GitHub release-candidate workflow

- Workflow: `Release candidate` (`.github/workflows/release.yml`).
- Run: [33921586860](https://github.com/Chenhzjs/GUI2TUI/actions/runs/33921586860).
- Event: manual `workflow_dispatch`.
- Source SHA: `efc704adf8a3ded3463ed8bb81670eddd08296c3`.
- Version job: PASS.
- Native Ubuntu 22.04 aarch64 build/package/smoke: PASS.
- Native Ubuntu 22.04 x86_64 build/package/smoke: PASS.
- Assembly, checksums and manifest: PASS.
- Build-provenance attestation: PASS for four subjects.
- Publish job: SKIPPED as required.

Workflow logs and `RELEASE-MANIFEST.json` independently identify the exact RC
source SHA. The successful final run supersedes diagnostic runs 33920347336,
33920503910 and 33921126106; none of those earlier candidates is qualified for
release.

GitHub emitted a non-blocking annotation that official Node 20 actions are
being forced to Node 24. The jobs passed; updating future action/runtime pins is
a P2 maintenance item, not a v0.3 release blocker.

## Extracted-package smoke

Both native jobs executed binaries extracted from their candidate archives in
fresh working and HOME/XDG directories. For each architecture:

```text
PACKAGE_VALUE_END_TO_END=PASS restored=true progress_read_only=true
PACKAGE_EXTERNAL_TEXT_END_TO_END=PASS authoritative_readback=true
PACKAGE_EXTERNAL_TEXT_CONFLICT_REFUSAL=PASS candidate_preserved=true
PACKAGE_EXTERNAL_TEXT_HANDLER_FAILURE=PASS gui_unchanged=true terminal_restored=true
PACKAGE_BACKING_FILE_BYPASS=ABSENT
PACKAGED_FRESH_HOME_SMOKE=PASS no_config=true action_confirmed=true password_absent=true broker_capabilities=true
RELEASE_VALIDATION=PASS version=0.3.0 smoke=true
```

The fresh-home smoke also covers version/help, startup/doctor, optional handler
absence, default spatial layout, `--layout flat`, a named semantic action,
password exclusion and local modality capabilities. The feature smoke covers
fresh Value read-back and restoration, read-only ProgressBar behavior,
complete-text write/read-back, conflict refusal with private candidate
preservation, handler failure without GUI mutation, and backing-file
separation.

## Artifact table

| Filename | Architecture | Bytes | SHA-256 | Maximum GLIBC | Architecture/content/smoke | Attestation |
| --- | --- | ---: | --- | --- | --- | --- |
| `gui2tui-0.3.0-linux-aarch64.tar.gz` | AArch64 | 15,071,007 | `5940fde48d7e988b3da70c43b9c072494820b9883a00242f10da71e65cff41e0` | 2.34 | PASS | PASS |
| `gui2tui-0.3.0-linux-x86_64.tar.gz` | x86-64 | 15,266,153 | `acbf43bc530e49d3aafa68987ae9ce9c1aa922c66267816688c519fb10c90c66` | 2.34 | PASS | PASS |

Fresh downloaded archives passed `sha256sum -c SHA256SUMS`. Each archive has
the established package layout, 251 regular files, zero symlinks and zero
world-writable regular files. The executable bit and ELF machine labels are
correct, and no fixture state, recovery candidate, user configuration, core
dump, token, credential, or private temporary file was found.

### SHA256SUMS

- Filename: `SHA256SUMS`.
- Size: 201 bytes.
- SHA-256: `bdb4536079074bca9261c3fd1ec8384799b55cfed0dd6090a6782ae467f75ef1`.
- Verification: PASS against fresh downloaded archive bytes.
- Attestation: PASS.

### Release manifest

- Filename: `RELEASE-MANIFEST.json`.
- Size: 693 bytes.
- SHA-256: `a1f0a1997828daf62adb5c135769423e095e48cbda3741fb62d2783b9411d715`.
- Version: `0.3.0`.
- Source: `efc704adf8a3ded3463ed8bb81670eddd08296c3`.
- Architectures: exactly `aarch64` and `x86_64`.
- Sizes and checksums: match the fresh downloaded artifacts.
- Verification and attestation: PASS.

## ABI, linkage and provenance

Both architectures require at most GLIBC 2.34, passing the unchanged project
gate of GLIBC <= 2.35. All three aarch64 binaries depend only on `libc.so.6`,
`libgcc_s.so.1` and `libm.so.6`. The x86_64 binaries additionally record
`ld-linux-x86-64.so.2`. No GLIBCXX requirement or unexpected build-host-only
runtime dependency was found.

`gh attestation verify` passed independently for both archives,
`SHA256SUMS`, and `RELEASE-MANIFEST.json`. GitHub run 33921586860 created one
build-provenance attestation for the four subjects and associated it with the
frozen source and existing release workflow. This proves provenance, not a
general absence of vulnerabilities.

## Frozen semantic and security regression

The extracted-package smoke and existing focused tests/evidence confirm:

- native single-line `EditSession` still requires authoritative read-back;
- Value is bounded by finite current/min/max/positive increment, uses a public
  setter and fresh `CurrentValue`, restores the controlled fixture, and leaves
  informational ProgressBar and navigation ScrollBar non-writable;
- complex text requires complete non-secret plain text, current generation and
  scope, a shell-free configured handler and a bounded private candidate;
- conflict is checked against fresh GUI text before public AT-SPI write-back;
- handler failure does not mutate the GUI and restores terminal ownership;
- candidate artifacts are private and bounded (0700 owned directories, 0600
  regular files, 256 KiB maximum), with link/owner/device/inode replacement
  checks retained;
- PasswordText cannot enter content or external interaction;
- `PartialRealized`/rich content remains non-whole-writable;
- the historically dangerous Qt multiline path remains quarantined and was not
  re-probed;
- stale generations, modal scope and anonymous actions remain rejected;
- no direct application backing-file path becomes mutation authority.

Source review found no production branch on application, process, window,
toolkit, browser, fixture or editor identity. Names appearing under `src/` are
limited to existing test/example data or comments, not capability decisions.
No DOM/CDP, UNO, private toolkit API, OCR/vision, keyboard/mouse injection,
coordinate clicking, XTest/uinput, action-index fallback, backing-file
mutation, or screenshot-derived semantic fallback was added.

## v0.2 regression and bounded application evidence

The extracted fresh-home smoke retained responsive spatial layout as default,
the flat compatibility fallback, semantic action read-back, password safety,
and external resource-modality availability. Existing focused tests and v0.3
phase evidence retain Region/F6 and pane navigation, Choice/commands,
Reader/Outline/Search, modal scope and the separation between resource External
Modality and semantic mutation.

Final-package application checks were deliberately bounded:

| Application/family | RC result |
| --- | --- |
| Mousepad 0.6.1 | PASS: extracted aarch64 package completed generic external edit and authoritative read-back; opened synthetic backing file remained byte-identical |
| Controlled GTK3/GTK4 | PASS: both package architectures passed core semantics, passwords and complex-text positive/conflict/failure contracts |
| Controlled Qt Value | PASS WITH SAFE LIMITATION: Slider 4 -> 5 -> 4; ProgressBar read-only; multiline quarantine untouched |
| EOG 45.3 | PASS: extracted aarch64 Inspector rendered graphical presentation with no writable ScrollBar Value noise |
| LibreOffice Writer 24.2.7 | READ-ONLY BY DESIGN for whole-target mutation: scene/content remained available and no external whole-document edit affordance appeared |
| Chromium | PASS WITH SAFE LIMITATION from established v0.3 evidence; fresh RC VM run NOT TESTED because Ubuntu supplied only an uninstalled Snap transition launcher |
| Firefox | PASS WITH SAFE LIMITATION from established v0.3 evidence; fresh RC run NOT TESTED because no executable was installed; unchanged authoritative write remains non-writable |
| GTK Demo | PASS WITH SAFE LIMITATION from current v0.3 evidence: explicit expand/collapse evidence did not qualify generic realization ownership |
| Qt Designer | PASS WITH SAFE LIMITATION from established evidence; NOT TESTED in the current RC VM because it was not installed |
| VS Code / Electron | PARTIAL from established evidence; no private Monaco/Electron integration and no fresh RC run required |

The unavailable browser/Designer repetitions are P2 evidence gaps rather than
package or capability failures: production Rust did not change after their
validated functional evidence, both package architectures passed the generic
semantic/safety harness, and no stronger application claim is made.

## P0, P1 and P2

- P0: none.
- P1: none.
- P2: some editors may replace the private candidate inode and remain
  unqualified; fresh Chromium, Firefox, Qt Designer and VS Code application
  repetitions were unavailable in the current RC VM; official GitHub actions
  reported the upcoming Node runtime transition. These are documented safe
  limitations or maintenance/evidence gaps, not failed required package gates.

## Release boundary

The exact RC source is suitable to become the immutable v0.3.0 release source:

```text
efc704adf8a3ded3463ed8bb81670eddd08296c3
```

No `v0.3.0` or `v0.3.0-rc1` tag was created, no tag was pushed, no GitHub
Release was created, and no package was publicly published. Existing release
tags remain immutable:

- `v0.1.0` -> `b4f4c530326cf5623bc75c9a16d54dfd55e6e81a`
- `v0.2.0` -> `578fcb2dcdfc07954587cc019caff2ba11982659`

The next separately authorized action is production release of v0.3.0 using
tag `v0.3.0` pointing **exactly** to the RC source above. Do not include the
evidence-only commit or any feature work in that tag.
