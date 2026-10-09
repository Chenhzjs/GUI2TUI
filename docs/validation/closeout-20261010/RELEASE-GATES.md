# Native candidate release gates

Qualified source commit: 44c8341 (release/product-completion-20261010).
Candidate version: 1.1.0-rc.1. No tag moved or created; no public release published.

- Source review covered exact identity/scope/tickets, text conflict/readback and
  partial mutations, remote path isolation, artifact cancellation and signal ownership.
  Preparation cancellation reporting was corrected before submission.
- CI passed: https://github.com/Chenhzjs/GUI2TUI/actions/runs/37961803697
- Native release gates passed: https://github.com/Chenhzjs/GUI2TUI/actions/runs/37961889345
- Ubuntu 22.04 native x86_64 and aarch64: build, ABI, extracted-package fresh-home
  smoke, checksums and upload passed. Both recorded PACKAGED_FRESH_HOME_SMOKE=PASS.
- Exactly two native archives assembled; manifest/checksum verification passed.
- Provenance: https://github.com/Chenhzjs/GUI2TUI/attestations/54396538
- Candidate artifact: gui2tui-1.1.0-rc.1-release-candidate (Actions retention 30 days).

Initial native run 37961220800 built both architectures and passed ABI but rejected
local developer paths in packaged documentation. The packaging fix normalizes only
bundle text; original repository evidence bytes and hashes remain unchanged. The full
native workflow was rerun successfully, without relaxing the validation gate.

This record is a later documentation commit, not the qualified source commit.
Remaining untracked historical runs in the developer workspace are retained locally;
required linked evidence, new user documentation and reproduction scripts are committed.
The earlier README's unsubmitted/unqualified wording describes its previous checkpoint.
