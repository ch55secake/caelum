from pathlib import Path
import plistlib
import tempfile
import unittest

from macos_app import stage


class DevBundleTests(unittest.TestCase):
    def test_rebuilds_keep_identity_and_checkouts_have_distinct_notification_owners(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            identities = []
            for name in ["first checkout", "other café"]:
                root = base / name
                assets = root / "apps/desktop/assets"
                assets.mkdir(parents=True)
                (assets / "icon.icns").write_bytes(b"icon")
                (root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.1.0"\n')
                binary = root / "target/debug/caelum-desktop"
                binary.parent.mkdir(parents=True)
                binary.write_bytes(b"first executable")
                binary.chmod(0o755)
                app = stage(root, binary)
                info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
                identities.append(info["CFBundleIdentifier"])
                self.assertNotEqual(info["CFBundleIdentifier"], "dev.caelum.desktop")
                self.assertNotIn("CFBundleURLTypes", info)
                executable = app / "Contents/MacOS/caelum-desktop"
                self.assertFalse(executable.is_symlink())
                self.assertEqual(executable.read_bytes(), b"first executable")
                binary.write_bytes(b"rebuilt executable")
                stage(root, binary)
                self.assertEqual(executable.read_bytes(), b"rebuilt executable")
                self.assertEqual(info, plistlib.loads((app / "Contents/Info.plist").read_bytes()))
            self.assertNotEqual(*identities)


if __name__ == "__main__":
    unittest.main()
