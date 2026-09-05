# GUI2TUI v0.3.0 production release verification

## Status and immutable identity

**GUI2TUI v0.3.0 PUBLICLY RELEASED**

**STRICT PRODUCTION VALIDATION PENDING — P2 x86_64 PUBLIC-DOWNLOAD NATIVE
SMOKE**

- Version: `0.3.0`.
- Release source: `efc704adf8a3ded3463ed8bb81670eddd08296c3`.
- Tag: `v0.3.0`.
- Tag target: `efc704adf8a3ded3463ed8bb81670eddd08296c3`.
- RC evidence HEAD: `04a436f3a8c57032a9b148354eeab07071ee96f9`.
- Post-release evidence HEAD: the documentation-only commit containing this
  record; it is not release source and does not change the tag.
- Release: <https://github.com/Chenhzjs/GUI2TUI/releases/tag/v0.3.0>.
- Published: 2026-09-05 05:40:42 UTC.
- Draft: false. Prerelease: false.
- P0: 0. P1: 0.

The tag target equals the qualified RC source. It does not point to the RC
evidence commit or the later post-release evidence commit.

## Existing tag integrity

Local and remote resolution after publication confirmed:

- `v0.1.0` -> `b4f4c530326cf5623bc75c9a16d54dfd55e6e81a`
- `v0.2.0` -> `578fcb2dcdfc07954587cc019caff2ba11982659`
- `v0.3.0` -> `efc704adf8a3ded3463ed8bb81670eddd08296c3`

`v0.3.0` was created as an annotated tag, matching the v0.2.0 style, and
pushed without force. All three public release tags are immutable.

## Production workflow

- Workflow: `Release candidate` (`.github/workflows/release.yml`).
- Run: [33947682181](https://github.com/Chenhzjs/GUI2TUI/actions/runs/33947682181).
- Trigger: push of `refs/tags/v0.3.0`.
- Head/source SHA: `efc704adf8a3ded3463ed8bb81670eddd08296c3`.
- Version gate: PASS (`0.3.0`, matching tag).
- Native Ubuntu 22.04 aarch64 build/package/smoke: PASS.
- Native Ubuntu 22.04 x86_64 build/package/smoke: PASS.
- Assembly, checksum and manifest verification: PASS.
- Build-provenance attestation: PASS, four subjects.
- Publish: PASS.

The workflow rebuilt the packages from the production tag. It did not promote
cached RC archive bytes or build the later branch/evidence HEAD.

## Production package smoke

The workflow executed each architecture's binaries from its newly extracted
production archive. Both native jobs recorded:

```text
PACKAGE_VALUE_END_TO_END=PASS restored=true progress_read_only=true
PACKAGE_EXTERNAL_TEXT_END_TO_END=PASS authoritative_readback=true
PACKAGE_EXTERNAL_TEXT_CONFLICT_REFUSAL=PASS candidate_preserved=true
PACKAGE_EXTERNAL_TEXT_HANDLER_FAILURE=PASS gui_unchanged=true terminal_restored=true
PACKAGE_BACKING_FILE_BYPASS=ABSENT
PACKAGED_FRESH_HOME_SMOKE=PASS no_config=true action_confirmed=true password_absent=true broker_capabilities=true
RELEASE_VALIDATION=PASS version=0.3.0 smoke=true
```

This covers package version, startup/doctor, absent optional handler, named
semantic action, password exclusion, responsive spatial default, flat fallback,
Value setter/read-back/restoration, read-only ProgressBar, complete text
write/read-back, conflict refusal, candidate preservation, handler failure with
no GUI mutation, terminal recovery and backing-file separation.

## Public release page

- URL: <https://github.com/Chenhzjs/GUI2TUI/releases/tag/v0.3.0>.
- Name: `GUI2TUI v0.3.0`.
- Tag: `v0.3.0`.
- Tag resolution: exact release source above.
- Draft: false.
- Prerelease: false.
- Assets: exactly ten expected files: two archives, their per-archive checksum,
  ABI and smoke records, `SHA256SUMS`, and `RELEASE-MANIFEST.json`.
- Notes: present and retain the qualified Verified Capability Recovery story.

After publication, the Release body received only a mechanical documentation
adjustment: its demonstration link now uses the correct tag-qualified public
path, and its final status sentence says that publication occurred. No release
source, artifact, tag, capability claim or release narrative changed.

## Fresh public-download verification

All ten assets were downloaded from the public GitHub Release into a new
verification directory. These public bytes, rather than workflow artifact or RC
cache paths, were used for the following checks.

| Filename | Architecture | Public bytes | Public SHA-256 | GLIBC max | Fresh checksum | ELF/content | Attestation | Production smoke |
| --- | --- | ---: | --- | --- | --- | --- | --- | --- |
| `gui2tui-0.3.0-linux-aarch64.tar.gz` | AArch64 | 15,071,007 | `5940fde48d7e988b3da70c43b9c072494820b9883a00242f10da71e65cff41e0` | 2.34 | PASS | PASS | PASS | PASS |
| `gui2tui-0.3.0-linux-x86_64.tar.gz` | x86-64 | 15,266,153 | `acbf43bc530e49d3aafa68987ae9ce9c1aa922c66267816688c519fb10c90c66` | 2.34 | PASS | PASS | PASS | PASS |

The public bytes happen to match the qualified RC hashes, but release
verification did not assume that result.

Each freshly extracted archive contains 251 regular files, zero symlinks and
zero world-writable regular files. The three expected executables have execute
permission and the correct ELF machine. Validation found no source checkout
path, user configuration, recovery candidate, fixture state, core dump,
credential, token or password content.

### SHA256SUMS

- Filename: `SHA256SUMS`.
- Public size: 201 bytes.
- SHA-256: `bdb4536079074bca9261c3fd1ec8384799b55cfed0dd6090a6782ae467f75ef1`.
- `sha256sum -c SHA256SUMS` against both fresh public archives: PASS.
- Attestation: PASS against the public bytes.

### Release manifest

- Filename: `RELEASE-MANIFEST.json`.
- Public size: 693 bytes.
- SHA-256: `a1f0a1997828daf62adb5c135769423e095e48cbda3741fb62d2783b9411d715`.
- Version: `0.3.0`.
- Source: `efc704adf8a3ded3463ed8bb81670eddd08296c3`.
- Architectures: exactly `aarch64` and `x86_64`.
- Artifact names, sizes and checksums match the public bytes: PASS.
- Attestation: PASS against the public bytes.

## ABI and linkage

Both production architectures require at most GLIBC 2.34 and pass the
established GLIBC <= 2.35 gate.

- aarch64: all three binaries are AArch64 ELF and depend on `libc.so.6`,
  `libgcc_s.so.1`, and `libm.so.6`.
- x86_64: all three binaries are x86-64 ELF and additionally record
  `ld-linux-x86-64.so.2`.
- No GLIBCXX requirement or unexpected build-host-only runtime dependency was
  found.

## Public-byte provenance

Independent `gh attestation verify` calls passed for:

- `gui2tui-0.3.0-linux-aarch64.tar.gz`;
- `gui2tui-0.3.0-linux-x86_64.tar.gz`;
- `SHA256SUMS`;
- `RELEASE-MANIFEST.json`.

Production workflow run 33947682181 created attestation 45413441 for the four
subjects. Provenance identifies the tag workflow and exact release source; it
does not by itself assert general vulnerability absence.

## Independent native public-download smoke

The freshly downloaded public aarch64 archive ran natively in the existing
Ubuntu 24.04 arm64 environment through `scripts/validate-release.sh --smoke`.
It independently repeated and passed all package markers above, including
`gui2tui 0.3.0`, Value restoration, complete-text authoritative read-back,
conflict refusal, handler failure, passwords, layouts and backing-file
separation.

The same public aarch64 package also completed a small real Mousepad 0.6.1
check through the normal generic configured-handler path. The GUI buffer
changed authoritatively while the synthetic file opened by Mousepad remained
byte-identical before application save.

An additional independent native x86_64 host was not naturally available. No
second public-download native execution is claimed. This is a P2 verification
gap under the established v0.2 release precedent; the production workflow's
native x86_64 extracted-package smoke, public checksum, manifest, ELF/ABI and
attestation all passed.

## Capability and security regression

- Native single-line text retains its verified `EditSession` path.
- Value uses finite advertised bounds/increment, public setter and independent
  `CurrentValue`; the fixture restored 4 -> 5 -> 4.
- ProgressBar remains informational and ScrollBar does not become writable
  Value noise.
- Complete complex text uses an optional configured direct-argv handler and a
  bounded GUI2TUI-owned candidate.
- Private interaction directories are 0700, files are 0600, and content is
  bounded to 256 KiB with link/owner/device/inode replacement checks.
- Fresh GUI text/generation/locator/scope checks occur before public AT-SPI
  write-back; full authoritative text read-back is mandatory afterward.
- Conflict leaves GUI state B unchanged and preserves candidate C privately.
- Handler failure performs no GUI mutation and restores the terminal.
- PasswordText never enters content, artifacts, handlers, logs or mutation.
- `PartialRealized` and rich/incomplete Writer content remain non-whole-writable.
- The unsafe Qt multiline Text path remains quarantined and was not re-probed.
- Anonymous actions and action-index guessing remain refused.
- External resource Modality remains separate from semantic text mutation.

No application/toolkit production branch, DOM/CDP, UNO, private toolkit API,
OCR/vision, screenshot semantics, keyboard/mouse injection, coordinate click,
XTest/uinput, anonymous-action fallback or direct backing-file mutation was
introduced.

## Documentation, demos and privacy

The tag source passed `scripts/check-docs.py` with 56 files and 177 local links.
README contains four v0.3 demo references. The hero/full videos and four public
screenshots decoded successfully from the exact tag source. The release notes
retain bounded Value/editor/application claims and the public demonstration
link resolves to the v0.3.0 tag content.

Focused review of the Release body, asset names, archives, demo media metadata,
README and tagged documentation found no credentials, tokens, personal
documents, developer checkout path, private hostname, password content or
private recovery artifact. Generic CI `/tmp` paths appearing in smoke logs do
not identify a developer or expose candidate content.

## Validation level and remaining P2

Public release status: **GUI2TUI v0.3.0 PUBLICLY RELEASED**.

Strict production validation status: **NOT YET FULLY VALIDATED — P2
VERIFICATION GAP**. The only missing check is an additional independent native
x86_64 execution using freshly downloaded public release bytes. This is not a
release blocker: the public release remains valid, and the production
workflow's native x86_64 package smoke plus public checksum, manifest,
architecture, ABI, and attestation checks all passed.

- P0: none.
- P1: none.
- P2: no additional independent native x86_64 public-download execution;
  inode-replacing external editors remain safely unqualified; GitHub official
  actions emitted the already-recorded Node runtime transition warning.

## Immutable release rule

`v0.3.0` **MUST NEVER MOVE**. The tag is public and immutable. This evidence
commit does not change it. Any future source fix belongs to v0.3.1 or later.
The project state is **POST-v0.3 MAINTENANCE**. Do not automatically begin v0.4.
