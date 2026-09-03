# GUI2TUI v0.2.0 public release verification

GUI2TUI v0.2.0 was published from the immutable, previously qualified RC
source commit.

- Release: <https://github.com/Chenhzjs/GUI2TUI/releases/tag/v0.2.0>
- Release source and tag target: `578fcb2dcdfc07954587cc019caff2ba11982659`
- Production workflow: [33713172945](https://github.com/Chenhzjs/GUI2TUI/actions/runs/33713172945)
- Published: 2026-09-03 04:00:15 UTC
- Release state: public, non-draft, non-prerelease
- `v0.1.0` remains `b4f4c530326cf5623bc75c9a16d54dfd55e6e81a`.

## Production workflow

The existing release workflow ran on the `v0.2.0` tag with publishing enabled.
Version, native x86_64 and aarch64 builds, extracted-package smoke tests,
assembly, the exact architecture gate, ABI, checksums, manifest and GitHub
artifact attestations all passed; publication passed.

## Published artifacts

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `gui2tui-0.2.0-linux-x86_64.tar.gz` | 13,417,642 | `5efe93ab8baab8866b9fce6a3dc478c8b33069e049aab54c9d7ac50097563081` |
| `gui2tui-0.2.0-linux-aarch64.tar.gz` | 13,228,484 | `8df9dceb9a4d9c1df30f2b547c1282a170234d688453f1c0b83c8b8f4ef9828b` |
| `SHA256SUMS` | 201 | `3479adc61c710f1ed3d4d306481ef216c63e7639b5f82a0c05513a22c2f340c3` |
| `RELEASE-MANIFEST.json` | 693 | `92283e84f23a33161e07a3549dd15722e05bc5269e06b41b4fd89bda58953e90` |

The manifest records version `0.2.0`, source commit
`578fcb2dcdfc07954587cc019caff2ba11982659`, and exactly `x86_64` and `aarch64`.
Both architectures report maximum GLIBC `2.34`.

## Independent public-download verification

The archives, `SHA256SUMS` and `RELEASE-MANIFEST.json` were downloaded again
from the public release into a fresh temporary directory. `sha256sum -c
SHA256SUMS` passed, and the manifest's versions, source, architecture set,
sizes and hashes matched the downloaded files. `gh attestation verify` returned
exit 0 for both archives, `SHA256SUMS` and `RELEASE-MANIFEST.json`.

## Public package smoke

- aarch64: PASS on a native Ubuntu arm64 (OrbStack) environment using the
  downloaded archive, fresh extraction and fresh-home smoke; `gui2tui --version`
  reported `gui2tui 0.2.0` and the packaged semantic/password checks passed.
- x86_64: the production workflow's native Ubuntu smoke passed and the public
  bytes were independently checksum/manifest/attestation verified. An
  independent native x86_64 host was not available in this macOS arm64
  environment, so a second native public-download smoke is not claimed.

The public binaries use the validated spatial/responsive layout by default;
`--layout flat` remains available as the documented compatibility fallback.

## Public content checks

The release page contains only the expected two architecture archives and the
workflow metadata files. README, documentation and demo links were checked;
the published release body describes v0.2 behavior without application-specific
claims. Focused privacy/path review found no developer paths, credentials,
passwords, tokens or private host data in the public release metadata or assets.

## Release boundary

No source code, tests or release infrastructure changed after tagging.
The `v0.2.0` tag is immutable and will not be moved. Future fixes belong to
v0.2.1 or later.

