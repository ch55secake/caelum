import hashlib
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
RENDERER = ROOT / "scripts/release/render_homebrew.py"
ASSETS = (
    "caelum-cli-v0.1.0-aarch64-apple-darwin.tar.gz",
    "caelum-cli-v0.1.0-x86_64-apple-darwin.tar.gz",
    "Caelum-v0.1.0-aarch64.dmg",
    "Caelum-v0.1.0-x86_64.dmg",
)


class RenderHomebrewTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary_directory.cleanup)
        self.root = Path(self.temporary_directory.name)
        self.tap = self.root / "tap"
        (self.tap / ".github/workflows").mkdir(parents=True)
        (self.tap / "Formula/a").mkdir(parents=True)
        (self.tap / "README.md").write_text("# tap\n")
        (self.tap / ".github/workflows/tests.yml").write_text(
            """name: Tests
jobs:
  audit:
    steps:
      - name: Audit formulae
        run: brew audit --new --strict --online --tap ch55secake/tap
      - name: Check formula style
        run: brew style Formula/a/example.rb
"""
        )
        self.checksums = self.root / "SHA256SUMS"
        self.checksums.write_text(
            "".join(f"{hashlib.sha256(asset.encode()).hexdigest()}  ./{asset}\n" for asset in ASSETS)
        )

    def run_renderer(self) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["python3", str(RENDERER), "0.1.0", str(self.checksums), str(self.tap)],
            capture_output=True,
            check=False,
            text=True,
        )

    def test_renders_formula_cask_and_tap_docs(self) -> None:
        result = self.run_renderer()

        self.assertEqual(result.returncode, 0, result.stderr)
        formula = (self.tap / "Formula/c/caelum-cli.rb").read_text()
        cask = (self.tap / "Casks/caelum.rb").read_text()
        workflow = (self.tap / ".github/workflows/tests.yml").read_text()
        readme = (self.tap / "README.md").read_text()

        self.assertIn('version "0.1.0"', formula)
        self.assertIn('license "MIT"', formula)
        self.assertIn('cask "caelum"', cask)
        self.assertNotIn("$AARCH64", formula + cask)
        self.assertIn("brew audit --cask --strict --online caelum", workflow)
        self.assertIn("Formula/c/caelum-cli.rb Casks/caelum.rb", workflow)
        self.assertIn("brew install --cask ch55secake/tap/caelum", readme)

    def test_repeated_release_updates_are_idempotent(self) -> None:
        first = self.run_renderer()
        second = self.run_renderer()

        self.assertEqual(first.returncode, 0, first.stderr)
        self.assertEqual(second.returncode, 0, second.stderr)
        readme = (self.tap / "README.md").read_text()
        workflow = (self.tap / ".github/workflows/tests.yml").read_text()
        self.assertEqual(readme.count("<!-- BEGIN CAELUM PACKAGES -->"), 1)
        self.assertEqual(workflow.count("brew audit --cask --strict --online caelum"), 1)

    def test_missing_checksum_fails_without_writing_package_files(self) -> None:
        self.checksums.write_text("")

        result = self.run_renderer()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(ASSETS[0], result.stderr)
        self.assertFalse((self.tap / "Formula/c/caelum-cli.rb").exists())


if __name__ == "__main__":
    unittest.main()
