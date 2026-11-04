import unittest

from macos import classify, cpu_seconds


class AttributionTests(unittest.TestCase):
    def test_interval_cpu_time_formats(self):
        self.assertAlmostEqual(cpu_seconds("1:02.34"), 62.34)
        self.assertAlmostEqual(cpu_seconds("2:03:04.50"), 7384.5)

    def test_only_owned_children_and_matching_webkit_coalitions_count(self):
        snapshot = {
            10: {"parent": 1, "executable": "/Caelum.app/Contents/MacOS/caelum-desktop"},
            11: {"parent": 1, "executable": "/XPC/com.apple.WebKit.WebContent"},
            12: {"parent": 1, "executable": "/XPC/com.apple.WebKit.GPU"},
            13: {"parent": 10, "executable": "/bin/codex"},
            14: {"parent": 1, "executable": "/bin/opencode"},
            15: {"parent": 13, "executable": "/bin/git"},
        }
        self.assertEqual(
            classify(snapshot, 10, 100, {11: 100, 12: 200}),
            {10: "native", 11: "webkit", 13: "provider", 15: "child"},
        )
        self.assertNotIn(11, classify(snapshot, 10, None, {11: None}))


if __name__ == "__main__":
    unittest.main()
