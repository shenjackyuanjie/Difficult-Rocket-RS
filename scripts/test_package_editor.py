"""便携 ZIP 的资源、路径与校验清单回归。"""
import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

import package_editor as package


class PackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.root = self.base / "源码"
        self.assets = self.base / "原版/assets"
        self.output = self.base / "输出 空格/试用.zip"
        self.editor = self.root / "target/release/dr-editor.exe"
        files = {
            self.editor: b"MZ-test-release",
            self.editor.with_suffix(".pdb"): b"omit-debug",
            self.root / "Cargo.toml": b"[workspace]",
            self.root / "Cargo.lock": b"# lock",
            self.root / "crates/dr-editor/src/main.rs": b"fn main() {}",
            self.assets.parent / "LICENSE": b"MPL-2.0",
            self.assets / "builtin/PartList.xml": b"<PartTypes/>",
            self.assets / "textures/parts/Pod.png": b"image",
            self.assets / package.FONT_DIR / "HarmonyOS_Sans_SC_Regular.ttf": b"font",
            self.assets / package.FONT_DIR / "LICENSE.txt": b"font-license",
            self.assets / "fonts/unrelated/huge.ttf": b"omit-font",
            self.assets / "ships/unrelated.xml": b"omit-ship",
        }
        files.update({self.assets / "ships" / name: b"<Ship/>" for name in package.SAMPLES})
        for path, content in files.items():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)

    def build(self):
        return package.build_package(self.root, self.editor, self.assets, self.output, "test-revision")

    def test_minimal_package_and_every_file_hash(self):
        before = {path: package.sha256(path) for path in self.assets.rglob("*") if path.is_file()}
        manifest = self.build()
        prefix = package.PACKAGE_ROOT + "/"
        with zipfile.ZipFile(self.output) as archive:
            self.assertIsNone(archive.testzip())
            self.assertEqual(len(archive.namelist()), len(set(archive.namelist())))
            saved = json.loads(archive.read(prefix + "build-manifest.json"))
            self.assertEqual(saved, manifest)
            self.assertEqual(set(archive.namelist()),
                             {prefix + name for name in manifest["sha256"]} | {prefix + "build-manifest.json"})
            for name, expected in manifest["sha256"].items():
                self.assertEqual(hashlib.sha256(archive.read(prefix + name)).hexdigest(), expected)
            self.assertEqual(saved["samples"], list(package.SAMPLES))
            self.assertFalse(any(".pdb" in name or "unrelated" in name for name in archive.namelist()))
            launcher = archive.read(prefix + "Start.cmd")
            self.assertIn(b'pushd "%~dp0"\r\n', launcher)
            self.assertIn(b'--catalog "%~dp0assets\\builtin\\PartList.xml"', launcher)
            self.assertIn(b'--assets "%~dp0assets"', launcher)
            self.assertIn(b'--ship "%~dp0assets\\ships\\Solar.xml" %*', launcher)
            with zipfile.ZipFile(io.BytesIO(archive.read(prefix + "source.zip"))) as source:
                self.assertIn("crates/dr-editor/src/main.rs", source.namelist())
                self.assertIn("Cargo.lock", source.namelist())
                self.assertIn("LICENSE", source.namelist())
                self.assertFalse(any("target" in name for name in source.namelist()))
        self.assertEqual(before, {path: package.sha256(path) for path in before})
        sidecar = self.output.with_suffix(".zip.sha256").read_text(encoding="utf-8")
        self.assertEqual(sidecar, f"{package.sha256(self.output)}  {self.output.name}\n")

    def test_missing_runtime_resource_fails_before_creating_zip(self):
        (self.assets / package.FONT_DIR / "HarmonyOS_Sans_SC_Regular.ttf").unlink()
        with self.assertRaises(FileNotFoundError):
            self.build()
        self.assertFalse(self.output.exists())

    def test_missing_part_texture_directory_fails_before_creating_output(self):
        texture = self.assets / "textures/parts/Pod.png"
        texture.unlink()
        texture.parent.rmdir()
        with self.assertRaisesRegex(FileNotFoundError, "部件纹理目录"):
            self.build()
        self.assertFalse(self.output.exists())
        self.assertFalse(self.output.with_suffix(".zip.sha256").exists())
        self.assertFalse(self.output.parent.exists())

    def test_empty_part_texture_directory_fails_before_creating_output(self):
        (self.assets / "textures/parts/Pod.png").unlink()
        with self.assertRaisesRegex(FileNotFoundError, "部件纹理目录"):
            self.build()
        self.assertFalse(self.output.exists())
        self.assertFalse(self.output.with_suffix(".zip.sha256").exists())
        self.assertFalse(self.output.parent.exists())

    def test_existing_zip_is_preserved(self):
        self.output.parent.mkdir(parents=True)
        self.output.write_bytes(b"existing")
        with self.assertRaises(FileExistsError):
            self.build()
        self.assertEqual(self.output.read_bytes(), b"existing")


if __name__ == "__main__":
    unittest.main()
