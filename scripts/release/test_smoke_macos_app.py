import tempfile
import unicodedata
import unittest
from pathlib import Path
from unittest.mock import patch

from smoke_macos_app import create_projects


class SmokeProjectTests(unittest.TestCase):
    def test_fixture_paths_match_filesystem_unicode_canonicalization(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)

            def canonicalize(path, *, strict=False):
                return Path(unicodedata.normalize("NFD", str(path)))

            with patch.object(Path, "resolve", canonicalize):
                projects = create_projects(root)

            self.assertEqual(
                projects,
                [
                    Path(unicodedata.normalize("NFD", str(root / "first café #1"))),
                    Path(unicodedata.normalize("NFD", str(root / "second & 2%+?"))),
                ],
            )


if __name__ == "__main__":
    unittest.main()
