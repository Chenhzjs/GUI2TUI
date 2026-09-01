# Phase 4B release pipeline validation

PHASE 4B RELEASE PIPELINE SUPPLEMENT VALIDATED

Core IR structural changes: NONE. CORE ARCHITECTURE FROZEN FOR v0.1.
This document records the earlier Phase 4B non-publishing pipeline validation.
At that historical checkpoint, no release tag existed. GUI2TUI v0.1.0 was later
published from source commit `b4f4c530326cf5623bc75c9a16d54dfd55e6e81a`;
see the [final public release verification](release-v0.1.0-validation.md).

## Source and GitHub evidence

- Validated source: `eede5077114b048a9bd6230e6112e9e9a71ac109`.
- [Release validation run 33414098916](https://github.com/Chenhzjs/GUI2TUI/actions/runs/33414098916):
  `workflow_dispatch`, `master`, `publish=false`, attestation enabled, **success**.
- [Normal CI run 33414080972](https://github.com/Chenhzjs/GUI2TUI/actions/runs/33414080972): **success**.
- Release run created 2026-08-31 16:26:25 UTC and completed 16:32:32 UTC,
  including concurrency queue time; native jobs took 2m51s (arm) and 3m26s (x86).
- Both native build jobs and assembly succeeded; the publish job was **skipped**.
- No shared Cargo cache was configured: both release jobs built on fresh hosted runners.
- This evidence document is a subsequent documentation-only change, not part of those archive bytes.

| Runner label | Native architecture | OS | Three binaries' highest GLIBC requirement | Fresh package smoke |
| --- | --- | --- | --- | --- |
| ubuntu-22.04 | x86_64 | Ubuntu 22.04 | 2.34 | PASS |
| ubuntu-22.04-arm | aarch64 | Ubuntu 22.04 | 2.34 | PASS |

Rust 1.88.0, checked-in Cargo.lock, `cargo build --locked --release --bins`.
Default Cargo release profile retained; no new strip/LTO/panic policy.
The ABI gate is <=2.35. Actual measured 2.34 improves on the earlier local aarch64
2.39 requirement. Runtime smoke was on Ubuntu 22.04, not every distribution with glibc 2.34.

## Final archive metadata

| Archive | Bytes | ELF machine |
| --- | ---: | --- |
| gui2tui-0.1.0-linux-x86_64.tar.gz | 10,683,326 | Advanced Micro Devices X86-64 |
| gui2tui-0.1.0-linux-aarch64.tar.gz | 10,485,031 | AArch64 |

Actual combined SHA256SUMS:

```text
82c865cdced8f0da1bd1692eb552c5ffeaf478afcd3a0dce970ef3ff913661bd  gui2tui-0.1.0-linux-aarch64.tar.gz
7c0415cfb04badc75f966a0d028327c966d956a13927e0eb9c28ce399d78df0d  gui2tui-0.1.0-linux-x86_64.tar.gz
```

All three binaries on both architectures depend on `libc.so.6`, `libgcc_s.so.1`,
and `libm.so.6`; x86_64 also records `ld-linux-x86-64.so.2` in DT_NEEDED.
No GLIBCXX version requirement was found. Per-binary readelf/objdump results are
stored in each ABI.json and the external architecture-specific ABI report.

BUILD-INFO.json and RELEASE-MANIFEST.json agree on version 0.1.0, the above source
commit and native architecture. The manifest contains exactly the two archives,
their byte sizes, hashes and measured ABI. Both bundles have the same logical
layout, licenses, example configuration, documentation, three binaries, and packaged smoke harness.

## Real extracted-package smoke

Selected lines from the x86_64 job's saved transcript:

```text
ABI_GATE=PASS architecture=x86_64 glibc_max=2.34 elf=Advanced Micro Devices X86-64
PACKAGED_FRESH_HOME_SMOKE=PASS no_config=true action_confirmed=true password_absent=true broker_capabilities=true
RELEASE_VALIDATION=PASS archive=gui2tui-0.1.0-linux-x86_64.tar.gz version=0.1.0 smoke=true
```

Selected lines from the aarch64 job's saved transcript:

```text
ABI_GATE=PASS architecture=aarch64 glibc_max=2.34 elf=AArch64
PACKAGED_FRESH_HOME_SMOKE=PASS no_config=true action_confirmed=true password_absent=true broker_capabilities=true
RELEASE_VALIDATION=PASS archive=gui2tui-0.1.0-linux-aarch64.tar.gz version=0.1.0 smoke=true
```

Each harness executed extracted `bin/` programs from a fresh temporary working
directory with fresh HOME/XDG directories, isolated D-Bus, Xvfb and AT-SPI. Its GTK
fixture also came from the archive, not the source checkout. Assertions cover
version/help, config validation and precedence, absent-config startup, diagnostics,
application selection, help overlay input confinement, keyboard Button action,
independent Inspector checked/status confirmation, password and input-log absence,
disabled mouse, fixture restart, and broker capabilities.

## Download and provenance verification

GitHub artifacts:

- `native-x86_64` (ID 9766496206).
- `native-aarch64` (ID 9766475017).
- `gui2tui-0.1.0-release-candidate` (ID 9766502384).

The assembled artifact was downloaded from GitHub to a fresh directory. Both final
archive checksums passed again locally. Re-running `assemble-release.py` against
the downloaded files passed metadata/ABI/smoke agreement checks for both architectures.
An independent scan of every regular tar member, including ELF binary bytes,
also passed the developer/runner-checkout-path audit for both final archives.

```bash
sha256sum -c SHA256SUMS
# On macOS the equivalent command used was shasum -a 256 -c SHA256SUMS.
gh attestation verify gui2tui-0.1.0-linux-x86_64.tar.gz --repo Chenhzjs/GUI2TUI
gh attestation verify gui2tui-0.1.0-linux-aarch64.tar.gz --repo Chenhzjs/GUI2TUI
```

Both `gh attestation verify` checks succeeded. The verified statement identifies
the above source commit, `.github/workflows/release.yml@refs/heads/master`, GitHub-hosted
execution, and run 33414098916. Its subjects include both archives, SHA256SUMS and
RELEASE-MANIFEST.json. This proves provenance, not absence of software vulnerabilities.

## Quality and failure investigation

macOS arm64 (installed Rust 1.91.0) and OrbStack Ubuntu 24.04 arm64 (installed Rust
1.98.0) passed:

```bash
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
git diff --check
```

Local commands selected those installed compilers with `RUSTUP_TOOLCHAIN=stable`;
GitHub CI and both release builds used the fixed Rust 1.88.0. Rust tests remain
223 (218 library + 2 inspector CLI + 3 user CLI), with no new Rust tests in this
packaging-only supplement. Six new standard-library Python assembly tests additionally
cover final bytes, missing/extra archives, smoke failure, commit mismatch and bundled
ABI mismatch (229 tests across both suites). YAML parse, actionlint 1.7.7, shell syntax, Python compile,
ABI gates, final-package privacy checks, two native live smoke runs and assembly checks passed.

### Bug analysis: hosted-runner readiness

- Symptom/reproduction: first release run 33411766661 failed the aarch64 no-app
  PTY assertion after a fixed 1.2-second sleep; the frame was still blank.
- Root cause: `release_smoke.py` assumed elapsed time implied semantic readiness.
  No product capability assertion had failed; startup had not completed.
- Minimal fix: commit 602839a replaced fixed startup checks with bounded semantic
  readiness waits, including fixture registration, selector and post-action state.
- Regression: the entire dual-native pipeline was rerun, not just the failed step;
  both extracted-package GTK state confirmations passed.
- Prevention: retain hard deadlines and independently verify resulting GUI state;
  never replace live smoke with a version-only check.

### Validation environment isolation

A later concurrent macOS/OrbStack quality run reused a shared target directory.
Linux replaced the top-level executable while macOS CLI tests were invoking it;
`file` identified that executable as Linux ELF. Linux passed and macOS was rerun
successfully after the competing build ended. A separate macOS target directory
was additionally used for clean validation. This was harness interference, not a
product-code defect. Use separate CARGO_TARGET_DIR values across OS environments.

### Bug analysis: embedded source paths

- Symptom: an independent all-bytes audit of the downloaded candidate found
  absolute runner checkout paths in all six ELF binaries despite the text audit passing.
- Root cause: grep's binary-ignore option omitted the executables; ordinary Rust
  release builds retain some source locations even without debug info.
- Minimal fix: the shared packaging command adds a checkout-path remapping flag;
  archive validation scans binary bytes too and reports filenames only.
- Negative regression: the new byte scan detects the old candidate's embedded paths.
  No core/product source or release optimization profile was changed.
- Prevention: keep binary-inclusive auditing and final-download verification; do not
  equate a text-only scan or successful smoke with complete release-content validation.

The earlier candidate run 33412224208 is **superseded** because of that path audit
failure. Run 33413938705 passed the remapping fix; the final run above also includes
the six assembly regression tests. Do not distribute the superseded candidate.

## Publication boundary

Manual validation defaults to no publication. Only a matching existing `v*` tag,
successful version/build/smoke/ABI/assembly gates and the publish condition can
reach the isolated contents-write job. Publication was intentionally not triggered
in this historical Phase 4B run; it was subsequently exercised successfully for
the immutable `v0.1.0` tag as recorded in the final verification report.

musl: NOT PURSUED. deb/rpm/AppImage/Flatpak: NOT IMPLEMENTED, OUT OF SCOPE.
Remote production endpoint, new-TTY attach and Wayland capture: NOT IMPLEMENTED.
Phase 4C real-world compatibility sweep remains future work.

GitHub reported Node.js 20 deprecation annotations for official v4 checkout/artifact
actions, executing them on Node.js 24. They passed in this run; future action-runtime
updates and Ubuntu 22.04 runner retirement still require routine maintenance.
