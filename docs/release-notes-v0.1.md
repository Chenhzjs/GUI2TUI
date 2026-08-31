# GUI2TUI v0.1.0

GUI2TUI translates Linux AT-SPI accessibility semantics into a terminal-native interface.

Artifacts are provided for Linux x86_64 and Linux aarch64. Verify them with:

```bash
sha256sum -c SHA256SUMS
gh attestation verify gui2tui-0.1.0-linux-ARCH.tar.gz --repo Chenhzjs/GUI2TUI
```

The ABI report and release manifest record the actually measured GLIBC requirement. A provenance attestation describes where an artifact was built; it is not a claim that the artifact is free of vulnerabilities.

Known limitations: Wayland static capture, a remote production companion, new-TTY attachment, continuous live graphics, and native deb/rpm/AppImage/Flatpak packages are not implemented. Accessibility-limited controls remain explicitly read-only rather than using guessed actions.
