#!/usr/bin/env python3
import hashlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "tools" / "stage_release.py"


class StageReleaseTests(unittest.TestCase):
    def make_artifacts(self, names, *, empty: set[str] | None = None) -> Path:
        empty = empty or set()
        root = Path(tempfile.mkdtemp())
        for name in names:
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            if name in empty:
                path.write_bytes(b"")
            else:
                path.write_bytes(name.encode() + b"\n")
        return root

    def run_script(self, artifacts: Path, output: Path) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--artifacts", str(artifacts), "--output", str(output)],
            text=True,
            capture_output=True,
            check=False,
        )

    def test_exact_three_installers_plus_manifest(self):
        root = self.make_artifacts(
            [
                "windows/nsis/HiQNet POS_0.1.1-rc.1_x64-setup.exe",
                "linux/deb/HiQNet POS_0.1.1-rc.1_amd64.deb",
                "linux/appimage/HiQNet POS_0.1.1-rc.1_amd64.AppImage",
            ]
        )
        output = Path(tempfile.mkdtemp()) / "out"
        result = self.run_script(root, output)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(list(output.iterdir())), 4)
        self.assertTrue((output / "SHA256SUMS.txt").is_file())
        self.assertEqual((output / "SHA256SUMS.txt").read_text().count("\n"), 3)

    def test_missing_installer_fails(self):
        root = self.make_artifacts(["windows/nsis/setup.exe"])
        output = Path(tempfile.mkdtemp()) / "out"
        result = self.run_script(root, output)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("falta", result.stderr)

    def test_duplicate_installer_fails(self):
        root = self.make_artifacts(
            [
                "a/one.exe",
                "b/two.exe",
                "linux/deb/x.deb",
                "linux/appimage/x.AppImage",
            ]
        )
        output = Path(tempfile.mkdtemp()) / "out"
        result = self.run_script(root, output)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("multiples", result.stderr)

    def test_empty_installer_fails(self):
        root = self.make_artifacts(
            [
                "windows/nsis/setup.exe",
                "linux/deb/x.deb",
                "linux/appimage/x.AppImage",
            ],
            empty={"windows/nsis/setup.exe"},
        )
        output = Path(tempfile.mkdtemp()) / "out"
        result = self.run_script(root, output)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("vacio", result.stderr)


if __name__ == "__main__":
    unittest.main()
