# GUI2TUI v1.0 PHASE 1.0C HANDOFF

## 1. Final status

**PHASE 1.0C FINAL QUALIFICATION & RELEASE READINESS VALIDATED — V1.0.0 RC READY**

This is an internal qualification conclusion. `RC READY` is not `RC CREATED`:
no version bump, RC tag, release tag, GitHub Release, public upload or push
was performed. The next authorized boundary is a separately approved
v1.0.0 Release Candidate preparation.

## 2. Git and source identity

- Starting HEAD: `dc8c115` (`docs: validate v1.0B stabilization soak`).
- Candidate source: `7494f6a8520a698ab3b1713a3b3ee01bc147cc42`
  (`docs: freeze v1.0 candidate support contract`).
- Final HEAD: `74a12f0` (`docs: close v1.0C final qualification`).
- Branch: `v1.0/integration-stabilization`.
- Worktree: clean at final close.
- Remote: no upstream is configured for this branch; no push was performed.
  Existing `origin/v0.7/deployment-environment-completeness` was not changed.
- Published tags `v0.1.0`, `v0.1.1`, `v0.2.0` and `v0.3.0` were not moved.

The candidate source contains no Rust, Cargo, installer, helper, package
builder or runtime production change relative to the validated 1.0B payload.
Before candidate freeze it added the support-contract and release-readiness
documents listed below. Post-candidate changes are this evidence handoff and
roadmap status only; they do not change production/package behavior.

## 3. Candidate source and provenance

All candidate archives were built from the same exact source commit:
`7494f6a8520a698ab3b1713a3b3ee01bc147cc42`.

| Item | Evidence |
| --- | --- |
| Package version | `0.3.0` |
| Cargo.lock SHA-256 | `27943b4878fbc02f5a8f8d67b4fd74df3baf0cc5ed62ebb888859fe9f9ce5b6e` |
| Rust | `rustc 1.88.0 (6b00bc388 2025-06-23)` |
| Cargo | `cargo 1.88.0 (873a06493 2025-05-10)` |
| Build pipeline | `scripts/package-linux.sh`, `Dockerfile.v07e-package` |
| Runner baseline | `ubuntu-22.04-container` |
| Candidate result directory | `/tmp/gui2tui-v10c-candidate-7494f6a8` |

The package builder used locked dependencies, release profile, deterministic
archive metadata and source-path remapping. Both BUILD-INFO records and both
ABI records name the same candidate commit.

## 4. Candidate package identity

| Target | Archive bytes | SHA-256 | ELF / glibc | Interpreter |
| --- | ---: | --- | --- | --- |
| GNU/Linux aarch64 | 15,604,967 | `02ccec3f00546c68e5f0617c2d2640baee1ab1827e1d6b2e8bafcca7bd42e559` | AArch64 / 2.34 | `/lib/ld-linux-aarch64.so.1` |
| GNU/Linux x86_64 | 15,829,851 | `1026332f20b2b7f0fb963bd0394e9cd98dac99f350bed79392675f897a476368` | AMD64 / 2.34 | `/lib64/ld-linux-x86-64.so.2` |

Both packages passed the declared glibc `2.35` gate. `glibcxx_max` is
`null`/none; no accidental C++ runtime dependency was reported. Dynamic
dependencies are the expected libc, libgcc, libm and architecture interpreter
set recorded in each `DEPENDENCIES.txt`.

Executable SHA-256 values from the extracted candidate packages:

- x86_64 `bin/gui2tui`: `7f4d90ce2b495b9a4712da1b2dae1cfe038a5f21e5dd9832c410b953d1f49a6b`.
- aarch64 `bin/gui2tui`: `5ed9f3572ced0e61a316377d280ac5b26792a42d9e56f19315d5d772a8f08688`.

## 5. Archive audit

Both archives passed `scripts/validate-release.sh --smoke`, checksum
validation and `scripts/assemble-release.py`. Each archive contained exactly
356 members (296 regular files and 60 directories), one safe top-level
directory, and no absolute/parent paths, duplicate members, links, special
files, group/world-writable entries, `.DS_Store`, `__MACOSX`, AppleDouble,
private-key-looking files, token/secret sentinels or validation-only temporary
artifacts. The independent candidate archive audit also passed these checks.

The packaged smoke harness returned `PACKAGED_FRESH_HOME_SMOKE=PASS` for both
architectures. The assembly manifest and SHA256SUMS matched the archive bytes.

## 6. Architecture and environment support contract

The candidate matrix is recorded in
[final-support-matrix.md](../final-support-matrix.md). Its final boundaries
are:

| Environment | Final candidate status |
| --- | --- |
| Managed Xvfb | Supported within the recorded Ubuntu 24.04 arm64/user-session boundary |
| Docker/OCI Headless interactive PTY | Supported with explicit one-topology limitations |
| Same-host SSH -> Managed | Supported with explicit loopback OpenSSH/client limitations |
| Headless Native Wayland | Supported with explicit Weston 13/Pixman/GTK4/Qt6 limitations |
| Headless XWayland | Supported with explicit Weston/XWayland 23.2.6/GTK limitations |
| GNU/Linux aarch64 | Qualified by native package/live evidence on the recorded arm64 host |
| GNU/Linux x86_64 | Package/ABI qualified with emulated-runtime limitation; native x86_64 not qualified |
| Linux native VT | Not qualified / not tested |
| Ordinary X11 desktop | Optional, not qualified |
| Ordinary Wayland desktop | Optional, not qualified |
| SSH -> existing desktop | Not qualified / not tested |
| Wayland over SSH | Not qualified / not tested |
| Wayland static capture | Unsupported / deferred |
| Cross-host Remote Companion | Post-1.0 |

No `NOT TESTED` row was promoted to `SUPPORTED`. No native x86_64 machine was
naturally available, and no external infrastructure or permissions were
expanded to obtain one.

## 7. Installation, replacement and uninstall

The exact candidate package qualification passed for both architectures:

- non-root default user-prefix installation;
- launch from an arbitrary working directory outside the checkout;
- required helper resolution and Doctor diagnostics;
- Managed setup, descriptor validation, clean stop and missing-session
  recovery;
- custom prefix containing spaces;
- packaged fresh-home smoke;
- safe uninstall preserving configuration and unrelated prefix files.

The non-overwriting installer was additionally exercised through the
documented replacement simulation on the exact aarch64 archive:

```text
existing user-prefix install
-> stop/recorded uninstaller
-> configuration and unrelated prefix file preserved
-> candidate install at the same prefix
-> version/launch check outside checkout
-> safe uninstall
```

Result: `REPLACEMENT_SIMULATION=PASS`. The installer continues to refuse
existing or changed targets. This is intentional and is now the upgrade
contract; it does not silently relax ownership, manifest or hash checks.

## 8. Final candidate task matrix

`v07e_package_qualification.sh` ran the exact candidate aarch64 bundle through
the installed TUI and fresh public readback in Docker PTY, same-host SSH and
Wayland/XWayland representative sessions. The 1.0A task evidence was then
carried forward only after the source-identity audit showed no changes to
`src/`, `Cargo.toml`, `Cargo.lock`, `scripts/install-user.sh`,
`scripts/uninstall-user.sh`, `scripts/package-linux.sh`, fixtures or package
builder between the final 1.0A production payload and this candidate. This
avoids mechanically repeating the approved common-task campaign while keeping
the evidence distinction explicit.

| Task | Candidate result | Evidence classification |
| --- | --- | --- |
| Install -> session -> application selection -> first scene | PASS | Direct exact-candidate package/install and Docker/SSH/Wayland evidence |
| Structured Form | PASS WITH SAFE LIMITATION | 1.0A final-package task evidence; unchanged relevant production payload; safe Qt transient-choice refusal remains documented |
| Single Selection | PASS | Direct exact-candidate Docker/SSH installed TUI plus authoritative readback |
| Table row | PASS | 1.0A final-package evidence; unchanged semantic/task implementation; current candidate package selection integration passed |
| PageTab | PASS | 1.0A final-package evidence carried by source-identity audit |
| Hierarchy | PASS | 1.0A final-package evidence carried by source-identity audit |
| Menu / Dialog / Modal | PASS | 1.0A final-package evidence carried by source-identity audit; Wayland modal smoke direct |
| Open File | PASS | 1.0A final-package ordinary-TUI evidence carried by source-identity audit; no chooser or production code changed |
| Choose Folder | PASS | 1.0A final-package ordinary-TUI evidence carried by source-identity audit; no chooser or production code changed |
| Reader | PASS | 1.0A final-package evidence carried by source-identity audit |
| Value | PASS | Exact candidate packaged smoke and 1.0A final-package evidence |
| Single-line Text | PASS | Exact candidate packaged smoke and 1.0A final-package evidence |
| Qualified complex Text | PASS | Exact candidate Docker/SSH/Wayland handler/writeback/readback evidence |
| Dynamic continuation | PASS | Exact candidate Docker/SSH/Wayland refresh/reconnect evidence |
| Stale authority refusal / fresh reselection | PASS | Exact candidate restart/replacement evidence; old locator rejected |
| Password safety | PASS / safe refusal | Exact candidate packaged sentinel smoke and source/security audit |
| Private artifact ownership | PASS | Exact candidate Docker/SSH/Wayland handler cleanup and archive audit |
| Clean terminal exit | PASS | Exact candidate Docker/SSH/Wayland terminal restoration and package smoke |

There are no unverified success claims in this matrix. The Qt transient
ComboBox shape remains `UNSUPPORTED BY PUBLIC SEMANTICS`, as established in
1.0A; it is not converted into a false pass.

## 9. Terminal and recovery smoke

The exact candidate package returned PASS for representative Docker PTY,
same-host SSH PTY and Weston headless/XWayland runs:

- ordinary navigation, Tab/Shift-Tab, arrows, Enter/Escape and enhanced-key
  delivery remained observable;
- semantic operation was followed by fresh authoritative readback;
- terminal resize and redraw passed in the Wayland representative run;
- modal scope, external handler return, clean quit and terminal restoration
  passed;
- application restart/replacement rejected old authority and required fresh
  explicit selection;
- SSH disconnect preserved the Managed session within the documented boundary;
- Wayland static capture remained an explicit unsupported result.

The formal 1.0B 2,700.25-second soak remains applicable because no Rust,
installer, helper, runtime, package-builder or terminal-lifecycle production
payload changed after that validated payload. The exact candidate additionally
passed the shorter direct package runtime smokes above.

## 10. Scale and performance

The exact candidate aarch64 executable was installed in a disposable Managed
Xvfb session with a generic controlled GTK4 tree. The fixture was mounted only
for validation and was not added to the repository or package. A public
Accessibility recursive walk produced:

- 3,904 semantic nodes;
- 3,904-line-scale inspector output, including 1,250 repeated item rows;
- traversal time `8,878.704 ms`;
- no fabricated unrealized nodes or unbounded expansion observed.

This is a bounded initial acquisition measurement, not an ordinary settled
redraw or operation/readback latency claim. It closes the prior current-source
319-node evidence gap with a larger generic tree, while leaving application
and environment variance explicit.

The release qualification budgets are:

- ordinary backend operation plus authoritative readback: configured 5,000 ms
  deadline, never indefinite;
- settled Command Palette/ordinary redraw on qualified core fixtures: <= 1 s;
- tested resize redraw and target reachability: <= 1 s on supported sizes;
- overflow recovery: bounded by the configured transition/operation deadline
  and reports timeout explicitly;
- soak resources: no monotonic FD/thread/child/artifact/queue growth and
  quiescent active operations return to zero;
- large/partial content: bounded traversal, explicit partial/truncation and
  no fabricated content.

The 3,904-node acquisition is not used to promise that arbitrary GUI
applications bootstrap in one second. It is within the bounded tree contract
and did not contradict an exit gate.

## 11. Security, privacy and genericity audit

The candidate source and package audit found no application-name, window-title,
toolkit, compositor, DOM/CDP, UNO, OCR, screenshot-semantic, coordinate-click,
keyboard/mouse-injection, action-index, fuzzy-target or backing-file bypass.

The following invariants remain intact:

- exact BackendLocator, current scope, capability and operation ticket;
- ApplicationGeneration isolation and stale-result refusal;
- fresh public Accessibility authoritative readback;
- PasswordText exclusion from cache, renderer, logs and handlers;
- private candidate/artifact directories and files with bounded ownership;
- direct-argv external handler with no shell interpolation;
- one terminal owner/reader and normal terminal restoration;
- no non-idempotent action replay after resize, event overflow or recovery.

## 12. Documentation qualification

The candidate documentation set was audited with `python3 scripts/check-docs.py`
and a clean user-prefix walkthrough of install, Doctor, Managed setup,
representative semantic smoke, stop and uninstall. The public documentation
now states the Headless-first topology rather than claiming generic Linux
desktop support, gives the non-overwriting replacement procedure, preserves
the x86_64 emulation limitation and names untested VT/desktop/SSH rows.

Prepared documents:

- [final support matrix](../final-support-matrix.md);
- [draft RC checklist](../rc-checklist.md);
- [unreleased v1.0 release notes draft](../../../release-notes-v1.0.0-draft.md).

The release notes remain explicitly draft/unreleased. No release metadata was
mutated.

## 13. Quality matrix

Executed against candidate source `7494f6a...` before package generation:

```text
cargo fmt --all -- --check                         PASS
cargo check --all-targets --locked                 PASS
cargo test --all-targets --locked                  PASS
  300 library tests, 2 inspector tests, 5 user CLI tests; 0 failed
cargo clippy --all-targets --locked -- -D warnings PASS
python3 scripts/check-docs.py                      PASS (100 files, 340 links)
git diff --check                                   PASS
```

Package build x2, archive validation, ABI audit, fresh install, replacement
simulation, uninstall, exact-candidate environment smokes and the generic
large-tree measurement also passed. The final documentation-only close reran
the documentation audit and diff check.

## 14. Severity classification

### P0

None. No wrong-target mutation, authority migration, password/private-content
exposure, unsafe artifact, false operation success or unrecoverable terminal
state was observed.

### P1

None. No supported candidate topology failed installation, core semantic smoke,
recovery, package provenance, safe replacement procedure or documentation
walkthrough.

### P2 / explicit limitations

- Native x86_64 hardware and a full native x86_64 environment matrix remain
  unqualified; the package/runtime result is explicitly emulation-limited.
- Linux native VT, ordinary X11/Wayland desktop, SSH to an existing desktop
  and Wayland over SSH remain not tested.
- The 3,904-node controlled acquisition took 8.879 s; it is a bounded startup
  measurement, not a one-second bootstrap promise for arbitrary applications.
- Public Accessibility omissions, partial realization, rich-text fidelity,
  pointer-only interactions, exhaustive virtualization and the Qt transient
  choice shape remain safe limitations.

These are not hidden release blockers because the support contract excludes
or explicitly limits them.

## 15. Release state and precise next boundary

- Package version: `0.3.0` unchanged.
- Public tags: unchanged and immutable.
- RC tag: not created.
- v1.0.0 tag: not created.
- GitHub Release/public artifacts: not created or uploaded.
- Push: not performed.
- Production feature implementation: none during 1.0C.

**1.0C is validated.** The only recommended next scope is:

> **v1.0.0 Release Candidate preparation**

That scope still requires a separate user authorization. This handoff stops
here and does not begin RC creation, tagging, version mutation or publication.
