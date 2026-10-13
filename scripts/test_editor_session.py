"""EXE 副本隔离 helper 单元测试：仅临时文件，无 GUI、键鼠、构建或桌面工具副作用。"""
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import editor_session


class CopyForRunTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "target/debug/dr-editor.exe"
        self.source.parent.mkdir(parents=True)
        self.source.write_bytes(b"MZ mock editor executable")
        self.output = self.root / "output"
        self.output.mkdir()

    def test_copy_is_byte_identical_and_source_stays_untouched(self):
        identity = editor_session.copy_for_run(self.source, self.output)
        copy_path = Path(identity["copy_path"])
        self.assertEqual(copy_path.read_bytes(), b"MZ mock editor executable")
        self.assertEqual(self.source.read_bytes(), b"MZ mock editor executable")
        self.assertEqual(identity["source_path"], str(self.source.resolve()))
        self.assertEqual(identity["source_sha256"], editor_session.file_sha256(self.source))
        self.assertEqual(identity["copy_sha256"], identity["source_sha256"])
        self.assertEqual(copy_path.parent, self.output.resolve())
        self.assertNotEqual(copy_path, self.source.resolve())

    def test_session_record_keeps_the_four_identity_fields(self):
        identity = editor_session.copy_for_run(self.source, self.output)
        session = json.loads((self.output / editor_session.SESSION_NAME).read_text(encoding="utf-8"))
        self.assertEqual(session, identity)
        self.assertEqual(sorted(session), ["copy_path", "copy_sha256", "source_path", "source_sha256"])

    def test_each_run_uses_a_unique_process_basename(self):
        identities = [editor_session.copy_for_run(self.source, self.output) for _ in range(3)]
        basenames = [Path(item["copy_path"]).name for item in identities]
        self.assertEqual(len(set(basenames)), 3)
        for basename in basenames:
            self.assertTrue(basename.startswith("dr-editor-"))
            self.assertTrue(basename.endswith(".exe"))
        elsewhere = self.root / "elsewhere"
        elsewhere.mkdir()
        moved = editor_session.copy_for_run(self.source, elsewhere)
        self.assertNotIn(Path(moved["copy_path"]).name, basenames)
        for item in identities:
            self.assertEqual(Path(item["copy_path"]).read_bytes(), self.source.read_bytes())

    def test_mismatched_copy_is_refused_and_never_handed_out(self):
        with mock.patch.object(editor_session, "file_sha256", side_effect=["0" * 64, "1" * 64]):
            with self.assertRaises(editor_session.SessionError) as raised:
                editor_session.copy_for_run(self.source, self.output)
        self.assertIn("SHA256", str(raised.exception))
        session = json.loads((self.output / editor_session.SESSION_NAME).read_text(encoding="utf-8"))
        self.assertNotEqual(session["source_sha256"], session["copy_sha256"])

    def test_missing_source_is_refused(self):
        with self.assertRaises(editor_session.SessionError):
            editor_session.copy_for_run(self.root / "missing.exe", self.output)


if __name__ == "__main__":
    unittest.main()
