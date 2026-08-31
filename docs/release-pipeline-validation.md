# Phase 4B release pipeline validation

PHASE 4B RELEASE PIPELINE SUPPLEMENT VALIDATED

Core IR structural changes: NONE. CORE ARCHITECTURE FROZEN FOR v0.1.
Public v0.1.0 release: NOT PUBLISHED. No release tag was created.

## Source and GitHub evidence

- Validated source: `602839aca6c7660d7d42bfff8926e0d49923cbd5`.
- [Release validation run 33412224208](https://github.com/Chenhzjs/GUI2TUI/actions/runs/33412224208):
  `workflow_dispatch`, `master`, `publish=false`, attestation enabled, **success**.
- [Normal CI run 33412195059](https://github.com/Chenhzjs/GUI2TUI/actions/runs/33412195059): **success**.
- Release run started 2026-08-31 16:06:23 UTC; duration 3m43s.
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
| gui2tui-0.1.0-linux-x86_64.tar.gz | 10,681,199 | Advanced Micro Devices X86-64 |
| gui2tui-0.1.0-linux-aarch64.tar.gz | 10,482,016 | AArch64 |

Actual combined SHA256SUMS:

```text
7fec900a3f69a8684a0981e49c83da1026024b879d1fd9d96990d48731ace551  gui2tui-0.1.0-linux-aarch64.tar.gz
4ee0ef5039eb8379b6650896a3c835c1a2457b01c6bc3320cbdff9ba0f50f0ff  gui2tui-0.1.0-linux-x86_64.tar.gz
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

- `native-x86_64` (ID 9765688055).
- `native-aarch64` (ID 9765670993).
- `gui2tui-0.1.0-release-candidate` (ID 9765698627).

The assembled artifact was downloaded from GitHub to a fresh directory. Both final
archive checksums passed again locally. Re-running `assemble-release.py` against
the downloaded files passed metadata/ABI/smoke agreement checks for both architectures.

```bash
sha256sum -c SHA256SUMS
# On macOS the equivalent command used was shasum -a 256 -c SHA256SUMS.
gh attestation verify gui2tui-0.1.0-linux-x86_64.tar.gz --repo Chenhzjs/GUI2TUI
gh attestation verify gui2tui-0.1.0-linux-aarch64.tar.gz --repo Chenhzjs/GUI2TUI
```

Both `gh attestation verify` checks succeeded. The verified statement identifies
the above source commit, `.github/workflows/release.yml@refs/heads/master`, GitHub-hosted
execution, and run 33412224208. Its subjects include both archives, SHA256SUMS and
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
packaging-only supplement. YAML parse, actionlint 1.7.7, shell syntax, Python compile,
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

## Publication boundary

Manual validation defaults to no publication. Only a matching existing `v*` tag,
successful version/build/smoke/ABI/assembly gates and the publish condition can
reach the isolated contents-write job. Tag publication: NOT TRIGGERED / NOT TESTED.
No public release or tag existed when checked after this run.

musl: NOT PURSUED. deb/rpm/AppImage/Flatpak: NOT IMPLEMENTED, OUT OF SCOPE.
Remote production endpoint, new-TTY attach and Wayland capture: NOT IMPLEMENTED.
Phase 4C real-world compatibility sweep remains future work.
