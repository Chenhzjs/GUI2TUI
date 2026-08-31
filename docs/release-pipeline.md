# Binary release pipeline

The dual-native pipeline is [live validated](release-pipeline-validation.md).
Public v0.1.0 release remains pending; workflow artifacts are validation candidates.

The normal CI workflow checks formatting, all targets, tests, Clippy, and patch whitespace.
It does not package or publish releases. The separate release workflow supports manual validation
and matching `v*` tags. Manual dispatch defaults to `publish=false`; publication additionally
requires a real tag whose value exactly matches the Cargo package version.

```text
CI (push/PR)                  Release validation (manual)
fmt/check/test/clippy        Ubuntu 22.04 x86_64 ─┐
no release permissions      Ubuntu 22.04 aarch64 ├─ ABI + extracted smoke
                                                   ↓
                                      checksums + manifest + attestation
                                                   ↓
                                      workflow artifact (no release)

Matching v* tag + every gate PASS + publish condition
                                                   ↓
                                      complete GitHub Release
```

Both native jobs call the same `scripts/package-linux.sh` used locally. `scripts/validate-release.sh`
checks the extracted layout, version, measured ABI, developer-path leakage and sentinel leakage,
then invokes only the packaged smoke harness. `scripts/assemble-release.py` requires both architectures,
successful smoke transcripts, matching commit/version metadata, and produces combined final checksums.

The fixed compiler is Rust 1.88.0 and every Cargo build uses `--locked`. ABI builds use
`ubuntu-22.04` and `ubuntu-22.04-arm`, not `ubuntu-latest`; the workflow still gates the measured
maximum GLIBC version at 2.35 rather than inferring it from a runner label.

## Verification

```bash
sha256sum -c SHA256SUMS
gh attestation verify gui2tui-0.1.0-linux-x86_64.tar.gz --repo Chenhzjs/GUI2TUI
gh attestation verify gui2tui-0.1.0-linux-aarch64.tar.gz --repo Chenhzjs/GUI2TUI
```

GitHub attestations require OIDC and repository attestation permission. No user-managed signing
secret is required. Provenance identifies the workflow and source commit; it does not certify that
the software is secure.

## Permissions

- CI: `contents: read` only.
- Build jobs: inherited `contents: read`; no OIDC or release writes.
- Assembly/attestation: `contents: read`, `id-token: write`, `attestations: write`.
- Publish: isolated job with `contents: write`, reachable only from a matching tag after all gates.

No public release is created by a default workflow dispatch. `deb`, `rpm`, AppImage, Flatpak and
musl artifacts are outside this pipeline. GitHub-hosted Ubuntu 22.04 runners have their own service
lifecycle; future retirement requires an explicit baseline decision, never an unnoticed switch to latest.
