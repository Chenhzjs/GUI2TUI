"""Small synthetic packaging tests; never a substitute for native live smoke."""
import hashlib
import io
import json
import pathlib
import subprocess
import sys
import tarfile
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "scripts/assemble-release.py"


class AssemblyTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = pathlib.Path(self.temp.name)
        for arch in ("x86_64", "aarch64"):
            prefix = f"gui2tui-0.1.0-linux-{arch}"
            abi = {"architecture": arch, "gui2tui_version": "0.1.0", "commit": "test-commit",
                   "glibc_max": "2.34", "elf_machine": arch}
            build = {"version": "0.1.0", "commit": "test-commit", "architecture": arch}
            with tarfile.open(self.root / f"{prefix}.tar.gz", "w:gz") as archive:
                for name, value in (("ABI.json", abi), ("BUILD-INFO.json", build)):
                    data = json.dumps(value).encode()
                    info = tarfile.TarInfo(f"{prefix}/{name}")
                    info.size = len(data)
                    archive.addfile(info, io.BytesIO(data))
            (self.root / f"{prefix}.abi.json").write_text(json.dumps(abi))
            (self.root / f"{prefix}.smoke.txt").write_text("PACKAGED_FRESH_HOME_SMOKE=PASS\n")

    def assemble(self):
        return subprocess.run([sys.executable, str(SCRIPT), str(self.root), "--version", "0.1.0",
                               "--commit", "test-commit"], capture_output=True, text=True)

    def test_complete_manifest_matches_final_bytes(self):
        result = self.assemble()
        self.assertEqual(result.returncode, 0, result.stderr)
        manifest = json.loads((self.root / "RELEASE-MANIFEST.json").read_text())
        self.assertEqual(len(manifest["artifacts"]), 2)
        for item in manifest["artifacts"]:
            data = (self.root / item["name"]).read_bytes()
            self.assertEqual(item["sha256"], hashlib.sha256(data).hexdigest())
            self.assertEqual(item["size_bytes"], len(data))

    def test_missing_architecture_blocks_assembly(self):
        (self.root / "gui2tui-0.1.0-linux-aarch64.tar.gz").unlink()
        self.assertNotEqual(self.assemble().returncode, 0)
        self.assertFalse((self.root / "RELEASE-MANIFEST.json").exists())

    def test_extra_archive_blocks_assembly(self):
        (self.root / "unexpected.tar.gz").touch()
        self.assertNotEqual(self.assemble().returncode, 0)

    def test_failed_smoke_blocks_assembly(self):
        (self.root / "gui2tui-0.1.0-linux-aarch64.smoke.txt").write_text("FAIL\n")
        self.assertNotEqual(self.assemble().returncode, 0)

    def test_wrong_commit_blocks_assembly(self):
        path = self.root / "gui2tui-0.1.0-linux-x86_64.abi.json"
        abi = json.loads(path.read_text())
        abi["commit"] = "other-commit"
        path.write_text(json.dumps(abi))
        self.assertNotEqual(self.assemble().returncode, 0)

    def test_bundled_abi_mismatch_blocks_assembly(self):
        path = self.root / "gui2tui-0.1.0-linux-x86_64.abi.json"
        abi = json.loads(path.read_text())
        abi["glibc_max"] = "2.39"
        path.write_text(json.dumps(abi))
        self.assertNotEqual(self.assemble().returncode, 0)


if __name__ == "__main__":
    unittest.main()
