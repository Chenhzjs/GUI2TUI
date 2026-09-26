# v1.0.0-rc.1 Internal RC Preparation Checklist

This checklist records internal RC preparation. It does not authorize an RC
tag, public publication or GitHub Release.

## Candidate identity

- [x] Freeze one exact source commit after all release-contract docs and
  validation scripts are complete — PASS: `3affc8f0268dccea016f96f269d8ede5d4d82862`.
- [x] Record Cargo.lock identity, Rust toolchain, build environment, target
  architecture, BUILD-INFO and source commit — PASS.
- [x] Build the aarch64 and x86_64 internal candidates from that same commit —
  PASS.
- [x] Record archive size, SHA-256, executable hashes and ABI reports — PASS.
- [x] Confirm package version is consistently `1.0.0-rc.1` for the internal
  candidate — PASS.

## Package and runtime

- [x] Archive validator passes with expected files only, safe modes, no
  developer paths, secrets, temporary evidence or unsafe links — PASS.
- [x] glibc symbol requirement remains within the 2.35 gate — PASS; maximum
  observed requirement is 2.34.
- [x] Fresh unprivileged install works at the default prefix and a prefix
  containing spaces, outside the source checkout — PASS.
- [x] Doctor, Managed setup, representative semantic smoke and clean stop
  pass on the declared topology — PASS.
- [x] Documented non-overwriting replacement path preserves configuration,
  runtime/recovery data and unrelated prefix files — PASS in the internal
  exact-candidate replacement simulation.
- [x] Safe uninstall removes only the recorded installation — PASS.

## Product and environment

- [x] Exact-candidate common-task evidence is complete by RC package smoke and
  carried-forward 1.0A final-package identity; Open File/Choose Folder and
  remaining rows were not mechanically rerun in this metadata-only pass.
- [x] Terminal/recovery smoke passes, including resize, modal, external
  handler, clean quit and terminal restoration — PASS.
- [x] Managed, Docker/PTY, same-host SSH and representative Wayland/XWayland
  claims match live evidence — PASS within the support matrix.
- [x] x86_64 wording explicitly retains the native/emulation boundary — PASS;
  native x86_64 remains unqualified.
- [x] Current-candidate scale/performance measurements and 1.0 budgets are
  recorded without relabelling historical evidence — PASS; RC measurement is
  6,405 realized nodes in 5,438.139 ms.
- [x] Security/genericity audit finds no forbidden semantic or authority
  bypass — PASS.

## Documentation and authorization

- [x] A clean-user walkthrough succeeds using only public documentation — PASS
  by the 1.0C walkthrough and RC install smoke.
- [x] Support matrix, limitations, upgrade instructions and release notes
  agree with evidence — PASS.
- [x] P0 = 0 and P1 = 0; remaining P2 items are explicit and non-blocking —
  PASS.
- [x] Full quality matrix and release manifest checks pass — PASS.
- [x] User separately authorizes RC creation before any version/tag/release
  operation — PASS as a release-safety condition; no such operation was
  performed.

## Explicit evidence boundaries

- Exact published v0.3.0 archive replacement replay: **NOT RUN — evidence
  limitation**. The documented internal non-overwriting replacement procedure
  passed with the exact RC candidate; no historical archive was reconstructed
  or downloaded.
- Reproducibility: **PASS within the declared fixed build image, source,
  target and Cargo target path**. Path-independent binary reproducibility is
  not claimed because a separate clean target/cache path changed ELF hashes.
- Host-side foreign-architecture CLI execution: **DEFERRED by design** on
  Darwin/arm64; relevant Linux package containers executed the checks.

## Final status

**V1.0.0 INTERNAL RELEASE CANDIDATE PREPARED AND QUALIFIED**

This checklist does not create an RC tag or authorize public publication.
