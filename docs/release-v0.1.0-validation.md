# GUI2TUI v0.1.0 public release verification

GUI2TUI v0.1.0 is publicly available from the immutable `v0.1.0` tag.

- Release: <https://github.com/Chenhzjs/GUI2TUI/releases/tag/v0.1.0>
- Release source: `b4f4c530326cf5623bc75c9a16d54dfd55e6e81a`
- Published: 2026-09-01 08:20:02 UTC
- Release state: public, non-draft, non-prerelease
- Repository state after release: **POST-v0.1 MAINTENANCE**

The tag points directly to the release source. Later documentation-only evidence
commits do not move it and are not part of the distributed binary archives.

## Validation pipeline

| Checkpoint | GitHub Actions run | Result |
| --- | --- | --- |
| Source CI | [33482990265](https://github.com/Chenhzjs/GUI2TUI/actions/runs/33482990265) | PASS |
| Non-publishing dual-architecture dry run | [33483179312](https://github.com/Chenhzjs/GUI2TUI/actions/runs/33483179312) | PASS; publish skipped |
| Tagged production release | [33486150680](https://github.com/Chenhzjs/GUI2TUI/actions/runs/33486150680) | PASS; release published |

The production workflow built natively on Ubuntu 22.04 x86_64 and aarch64,
smoke-tested both extracted packages, assembled exactly two architecture archives,
checked the GLIBC ceiling, generated checksums and provenance attestations, and
published only after the matching tag gates passed. Both archives require at most
GLIBC 2.34, within the release gate of GLIBC 2.35.

## Public assets

| Asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `gui2tui-0.1.0-linux-aarch64.tar.gz` | 11,449,073 | `5b934f6c8edb416e5ce8bb7114ee17da994a6a3435ee8a1e9a7cd52eb3243124` |
| `gui2tui-0.1.0-linux-x86_64.tar.gz` | 11,651,378 | `96a15b9119c91ba73f8fdb6900ae6164cfe2d56d4ee17a3d3d63d21d853e4408` |
| `SHA256SUMS` | 201 | `b879db776d3bcf023e1939809c568ac98031827612e44de962c0447ee04348c3` |
| `RELEASE-MANIFEST.json` | 693 | `f5f73ef4c037e316d4434b20d7e8f87a81e91024d28dff9db3630b3c3b2438d6` |
| `gui2tui-v0.1-demo.mp4` | 704,741 | `a9cd526d86583e44a16c561982011b52242b3aec7e98c9c522f6ac4005de75b9` |

The demo is a separately uploaded public presentation asset. It is not a binary
distribution subject and is intentionally outside the signed two-archive assembly
manifest. The release also contains per-architecture ABI and smoke reports.

## Independent public-download verification

All release assets were downloaded again from the public GitHub Release rather
than reused from a build workspace.

- `sha256sum -c SHA256SUMS`: PASS for both archives.
- `RELEASE-MANIFEST.json`: exact release source, architectures, sizes, hashes and
  GLIBC 2.34 ceilings confirmed.
- `gh attestation verify`: PASS for both archives, `SHA256SUMS`, and
  `RELEASE-MANIFEST.json`.
- Public aarch64 archive: clean extracted-package smoke PASS in an isolated
  Ubuntu 24.04 arm64 environment with Xvfb, D-Bus, AT-SPI and a fresh HOME.
- Public x86_64 archive: native Ubuntu 22.04 workflow smoke PASS; public bytes
  independently checksummed, matched to the manifest, and attestation-verified.

The fresh aarch64 public-package run confirmed diagnostics, AT-SPI Cache bootstrap,
semantic action/read-back, password absence, and broker capability reporting.

## Presentation verification

The GitHub README was inspected as rendered, including a narrow viewport. The
visual mark, five badges, real hero GIF and Mermaid architecture diagram loaded;
the narrow view had no horizontal overflow. Public release and demo links use the
stable `v0.1.0` URLs.

The 60-second demo is a real split-screen capture from an isolated Ubuntu 24.04
arm64 Xvfb/AT-SPI/GTK4 session. Its reproducible recording procedure and scope are
documented in [Demo recording](demo/README.md). The public assets contain no real
password, private document, credential, or developer-local absolute path.

## Release boundary

Core IR structural changes: **NONE**. Release polish changed documentation,
presentation assets, reproducible recording tooling, and a generic release-smoke
display-readiness check. The semantic/content/scene/modality/runtime contracts
remain frozen for v0.1.

Known limitations remain those stated in [Limitations](limitations.md). They are
safe, documented product boundaries rather than hidden compatibility claims.
