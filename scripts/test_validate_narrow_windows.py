"""窄窗验收契约测试；纯数据测试不启动 Win32 窗口。"""
import copy
import struct
import unittest

from validate_narrow_windows import is_editor_window, validate_dimensions


class NarrowWindowEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.size = (390, 844)
        self.window = {"requested_size": [390, 844], "logical_size": [390.0, 844.0],
                       "physical_size": [390, 844], "scale_factor": 1.0, "stable_size_frames": 3}
        self.png = b"\x89PNG\r\n\x1a\n" + struct.pack(">I", 13) + b"IHDR" + struct.pack(">II", *self.size)

    def test_only_owned_visible_nonempty_editor_windows_match(self):
        for title in ("Difficult Rocket Editor", "Test — Difficult Rocket Editor"):
            with self.subTest(title=title):
                self.assertTrue(is_editor_window(7, 7, True, title, self.size))
        for pid, visible, title, client in (
            (8, True, "Difficult Rocket Editor", self.size),
            (7, False, "Difficult Rocket Editor", self.size),
            (7, True, "辅助窗口", self.size),
            (7, True, "Difficult Rocket Editor", (0, 844)),
            (7, True, "Difficult Rocket Editor", (390, 0)),
        ):
            with self.subTest(pid=pid, visible=visible, title=title, client=client):
                self.assertFalse(is_editor_window(pid, 7, visible, title, client))

    def test_three_matching_dimensions_pass(self):
        self.assertEqual(validate_dimensions(self.size, [[390, 844]], self.window, self.png), self.size)

    def test_missing_or_wrong_client_fails(self):
        for clients in ([], [[1440, 900]], [[390, 844], [406, 860]]):
            with self.subTest(clients=clients), self.assertRaises(ValueError):
                validate_dimensions(self.size, clients, self.window, self.png)

    def test_each_bevy_dimension_must_match(self):
        for key in ("requested_size", "logical_size", "physical_size"):
            window = copy.deepcopy(self.window)
            window[key] = [1440, 900]
            with self.subTest(key=key), self.assertRaises(ValueError):
                validate_dimensions(self.size, [[390, 844]], window, self.png)

    def test_dpi_and_stability_must_match(self):
        for key, value in (("scale_factor", 2.0), ("stable_size_frames", 1)):
            window = copy.deepcopy(self.window)
            window[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                validate_dimensions(self.size, [[390, 844]], window, self.png)

    def test_original_png_cannot_be_desktop_sized(self):
        png = self.png[:16] + struct.pack(">II", 1440, 900)
        with self.assertRaises(ValueError):
            validate_dimensions(self.size, [[390, 844]], self.window, png)

    def test_invalid_or_truncated_png_fails(self):
        for png in (b"", self.png[:23], b"not-a-png" + self.png[9:], self.png[:12] + b"NOPE" + self.png[16:]):
            with self.subTest(png=png), self.assertRaises(ValueError):
                validate_dimensions(self.size, [[390, 844]], self.window, png)


if __name__ == "__main__":
    unittest.main()
