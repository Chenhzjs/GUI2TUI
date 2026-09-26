# v1.0.0-rc.1 Internal RC Preparation Checklist

This checklist records internal RC preparation. It does not authorize an RC
tag, public publication or GitHub Release.

## Candidate identity

- [ ] Freeze one exact source commit after all release-contract docs and
  validation scripts are complete.
- [ ] Record Cargo.lock identity, Rust toolchain, build environment, target
  architecture, BUILD-INFO and source commit.
- [ ] Build the aarch64 and x86_64 internal candidates from that same commit.
- [ ] Record archive size, SHA-256, executable hashes and ABI reports.
- [ ] Confirm package version is consistently `1.0.0-rc.1` for the internal
  candidate.

## Package and runtime

- [ ] Archive validator passes with expected files only, safe modes, no
  developer paths, secrets, temporary evidence or unsafe links.
- [ ] glibc symbol requirement remains within the 2.35 gate.
- [ ] Fresh unprivileged install works at the default prefix and a prefix
  containing spaces, outside the source checkout.
- [ ] Doctor, Managed setup, representative semantic smoke and clean stop
  pass on the declared topology.
- [ ] Documented non-overwriting replacement path preserves configuration,
  runtime/recovery data and unrelated prefix files.
- [ ] Safe uninstall removes only the recorded installation.

## Product and environment

- [ ] Exact-candidate common-task matrix passes with authoritative readback.
- [ ] Terminal/recovery smoke passes, including resize, modal, external
  handler, clean quit and terminal restoration.
- [ ] Managed, Docker/PTY, same-host SSH and representative Wayland/XWayland
  claims match live evidence.
- [ ] x86_64 wording explicitly retains the native/emulation boundary.
- [ ] Current-candidate scale/performance measurements and 1.0 budgets are
  recorded without relabelling historical evidence.
- [ ] Security/genericity audit finds no forbidden semantic or authority
  bypass.

## Documentation and authorization

- [ ] A clean-user walkthrough succeeds using only public documentation.
- [ ] Support matrix, limitations, upgrade instructions and release notes
  agree with evidence.
- [ ] P0 = 0 and P1 = 0; remaining P2 items are explicit and non-blocking.
- [ ] Full quality matrix and release manifest checks pass.
- [ ] User separately authorizes RC creation before any version/tag/release
  operation.
