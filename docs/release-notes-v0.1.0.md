# GUI2TUI v0.1.0 — release candidate draft (not published)

**NOT READY TO RELEASE v0.1.0.** See [Phase 4C validation](phase4c-validation.md)
and the [open issue ledger](validation/phase4c-issues.md).

GUI2TUI re-renders Linux accessibility semantics into terminal-native tasks and
Reader content; it is not a framebuffer-to-text desktop. Existing supported
paths include safe advertised actions, choice selection, atomic **single-line**
plain input editing, read-only document navigation/search, event-driven semantic
state, and explicitly authorized reference/static-image presentation.

The candidate fixes a real editor regression: multi-line document buffers no
longer enter the atomic single-line edit session and can use the existing Reader.
GTK/Qt controls and real Mousepad/Writer/browser workflows have fresh evidence.

Known release blockers: fresh-session Chrome Cache inventory can force a ~4 s
5K-node walk instead of the historical fast bootstrap; list/settings-heavy
interaction evidence is not yet complete. Real VS Code Reader/search has limited
evidence, not full editor compatibility. Writer
long-document coverage remains honestly PartialRealized.

Anonymous actions, unavailable choice options and unsafe text endpoints degrade
without guessed keyboard/mouse injection. Password editing, multiline editing,
remote production transport, new-TTY attach, Wayland capture/compositor and live
graphics are not implemented. No application-specific extraction is used.

Candidate artifact/checksum/attestation procedures remain documented in
[release engineering](release-pipeline-validation.md). A build attestation is
provenance, not proof that every workflow works or that a release is authorized.

Non-publishing pipeline [33419080074](https://github.com/Chenhzjs/GUI2TUI/actions/runs/33419080074)
passed for source `156149b5928d1ce607ba87fd0f6144f42f98f493`: both native Linux
architectures, extracted-package smoke, ABI and checksums. Publish was skipped.
These candidate artifacts are validation evidence, not an approved public release.
