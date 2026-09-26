# GUI2TUI v1.0.0 Final Release HANDOFF

## Final status

**GUI2TUI v1.0.0 RELEASED AND VERIFIED**

The final release gates passed before publication. No `v1.0.0-rc.1` tag was
created or published, and no existing public tag was moved.

## Source and Git identity

- Starting HEAD: `1f1e409662ff95ac947518ae2129b4b8b92f8fcf`
- Internal RC source: `3affc8f0268dccea016f96f269d8ede5d4d82862`
- Final release source: `d8abf9ae863dbfbef87e35310beecb8669f2863d`
- Final tag object: `e1b3902c6be3c0a24ce8bcef21a764ca591e02ee`
- `v1.0.0^{}`: `d8abf9ae863dbfbef87e35310beecb8669f2863d`
- Branch: `v1.0/integration-stabilization`
- Post-release documentation HEAD: `bb5e938d64b5c05049182a3bb9302e8e9ddcc772`;
  it is not the tag target.
- Remote branch and tag were pushed without force. The historical tags
  `v0.1.0` through `v0.3.0` remain unchanged.

The RC-to-final source delta was limited to version/release metadata, release
documentation, and the final release closeout. The audit found:

```text
UNEXPECTED_RUNTIME_DELTA=0
UNEXPECTED_SEMANTIC_DELTA=0
UNEXPECTED_INSTALLER_DELTA=0
UNEXPECTED_HELPER_DELTA=0
```

## Version, build, and artifacts

- Final version: `1.0.0`
- Cargo.lock SHA-256:
  `f350cf2612e3eacd4a00727ab288f2798849a8aade8cd749edc6884f4f20c591`
- Rust: `rustc 1.88.0 (6b00bc388 2025-06-23)`
- Cargo: `1.88.0 (873a06493 2025-05-10)`
- Build baseline: `ubuntu-22.04-container`
- Public workflow: GitHub Actions run `36271545143`, successful

The public release assets were downloaded and independently checked against
the published manifest and checksums:

| Artifact | Size | SHA-256 |
| --- | ---: | --- |
| `gui2tui-1.0.0-linux-aarch64.tar.gz` | 15,617,955 | `618f1d307204ef49c93538d2a1e05fafb6721ed90b9db2cfc77edc74fd015dc4` |
| `gui2tui-1.0.0-linux-x86_64.tar.gz` | 15,848,539 | `270fa8da900603a7fa80b809679a31bcd0f01f8b630ecbb43650c43f768bba6c` |

The locally qualified Docker-built archives have different bytes from the
public native-runner archives; this is expected and does not weaken the
qualification because the exact public assets were downloaded, checksum
verified, manifest verified, and independently audited after publication.

Both final archives passed one-root, inventory, path, mode, link, duplicate,
special-file, secret, and build-leakage checks. Both are the same version and
source commit according to the public `RELEASE-MANIFEST.json`.

ABI qualification passed for both targets. Maximum observed glibc requirement
is `2.34`, within the `<= 2.35` contract; no GLIBCXX dependency was observed.

## Qualification results

- Fresh final-package install and uninstall: PASS
- Final semantic smoke with authoritative readback: PASS
- Modal and stale-authority refusal: PASS
- Open File or Choose Folder representative chooser smoke: PASS
- External Text smoke and conflict safety: PASS
- PasswordText safe refusal: PASS
- Normal/narrow PTY, resize, Command Palette, clean quit, and terminal
  restoration: PASS
- Managed Xvfb: PASS
- Docker Headless interactive PTY: PASS
- Same-host SSH -> Managed: PASS
- Headless Native Wayland and XWayland representative smoke: PASS with the
  documented limitations
- 1.0B soak evidence remains applicable because final release contains no
  runtime behavior change
- Final x86_64 package/emulated smoke: PASS; native x86_64 remains not
  qualified

The full final-source quality matrix passed:

```text
cargo fmt --all -- --check                 PASS
cargo check --all-targets --locked         PASS
cargo test --all-targets --locked          PASS (300 library, 2 inspector, 5 CLI)
cargo clippy --all-targets --locked -- -D warnings  PASS
python3 scripts/check-docs.py              PASS (102 files, 341 local links)
git diff --check                           PASS
```

Security and genericity review found no application/toolkit branches, private
API path, OCR, injection, coordinate operation, fuzzy target migration,
backing-file mutation, password export, or unsafe artifact handling.

## Upgrade, support, and release state

The documented upgrade contract is explicit and fail-closed: stop Managed,
use the old installation's safe uninstall procedure while preserving
user-owned configuration/recovery data, install `1.0.0`, run Doctor, and
recreate or reselect the session/application where required. In-place blind
overwrite is not promised.

The final support contract remains narrow and evidence-based:

- Managed Xvfb, Docker Headless, and same-host SSH -> Managed: supported within
  the recorded topology boundaries.
- Native Headless Wayland and XWayland: supported with explicit limitations.
- GNU/Linux aarch64: qualified.
- GNU/Linux x86_64: package and emulated-runtime evidence; native hardware is
  not qualified.
- Linux native VT, ordinary desktop claims, and Wayland over SSH: not
  qualified/not claimed.
- Static capture and cross-host Remote Companion: unsupported/deferred beyond
  1.0.
- Accessibility limitations, partial realization, password exclusion, and
  unsupported rich or private semantics remain intentional product
  boundaries.

Formal release notes are published as `GUI2TUI v1.0.0` without an invented
static release date. The GitHub Release is public, non-draft, and non-prerelease:

<https://github.com/Chenhzjs/GUI2TUI/releases/tag/v1.0.0>

Published assets include both architecture archives, `SHA256SUMS`,
`RELEASE-MANIFEST.json`, ABI reports, and smoke summaries. Validation logs and
internal HANDOFF documents were not uploaded.

P0: **0**. P1: **0**. Remaining P2 items are documented non-blocking or
unsupported-scope limitations; none blocks the final release contract.

## Post-release closeout

The post-release documentation-only commit records this release, its exact
source/tag identity, hashes, support boundaries, and roadmap completion. It
does not change the tagged source, package payload, version, or release
assets. No v1.0.1/v1.1 implementation, Remote Companion work, or new feature
milestone was started.
