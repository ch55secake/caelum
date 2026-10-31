import hashlib
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from publish_release import (
    asset_names, installation_notes, parse_checksums, prepare_assets, publish,
    release_for_tag, verify_remote_assets,
)


class GitHubFixture:
    def __init__(self, tag):
        self.tag = tag
        self.release = None
        self.files = {}
        self.calls = []
        self.fail_upload = False
        self.corrupt_upload = False

    def snapshot(self, *_):
        if self.release is None:
            return None
        return {
            **self.release,
            "assets": [
                {"name": name, "size": len(data), "state": "uploaded",
                 "digest": f"sha256:{hashlib.sha256(data).hexdigest()}"}
                for name, data in self.files.items()
            ],
        }

    def gh(self, *args):
        self.calls.append(args)
        if args[:2] == ("release", "create"):
            self.release = {"id": 17, "draft": True}
        elif args[:2] == ("release", "upload"):
            for name in args[args.index("--clobber") + 1:]:
                file = Path(name)
                self.files[file.name] = file.read_bytes()
                if self.fail_upload:
                    self.fail_upload = False
                    raise subprocess.CalledProcessError(1, ["gh", *args])
            if self.corrupt_upload:
                self.files[f"Caelum-{self.tag}-aarch64.dmg"] = b"wrong uploaded bytes"
        elif args[:2] == ("release", "download"):
            name = args[args.index("--pattern") + 1]
            (Path(args[args.index("--dir") + 1]) / name).write_bytes(self.files[name])
        elif args[:3] == ("api", "--method", "PATCH"):
            self.release["draft"] = False
        else:
            raise AssertionError(f"Unexpected GitHub command: {args}")
        return ""


class PublishingTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.tag = "v0.1.6"
        for name in asset_names(self.tag):
            (self.directory / name).write_bytes(name.encode())
        self.github = GitHubFixture(self.tag)
        for name, replacement in [("gh", self.github.gh), ("release_for_tag", self.github.snapshot)]:
            mock = patch(f"publish_release.{name}", replacement)
            mock.start()
            self.addCleanup(mock.stop)

    def publish(self):
        publish("example/caelum", self.tag, self.directory)

    def test_upload_failure_leaves_a_recoverable_draft_and_only_complete_assets_publish(self):
        self.github.fail_upload = True
        with self.assertRaises(subprocess.CalledProcessError):
            self.publish()
        self.assertTrue(self.github.release["draft"])
        self.assertEqual(len(self.github.files), 1)
        self.publish()
        self.assertFalse(self.github.release["draft"])
        self.assertEqual(set(self.github.files), asset_names(self.tag) | {"SHA256SUMS"})
        self.assertEqual(sum(call[:2] == ("release", "create") for call in self.github.calls), 1)
        self.assertIn("make_latest=legacy", self.github.calls[-1])

    def test_published_releases_are_verified_and_reused_without_rebuilt_assets(self):
        self.publish()
        original = dict(self.github.files)
        self.github.calls.clear()
        publish("example/caelum", self.tag, self.directory / "no-build-needed")
        self.assertEqual(self.github.files, original)
        self.assertTrue(all(call[:2] == ("release", "download") for call in self.github.calls))

    def test_incomplete_or_corrupt_published_assets_fail_without_overwriting_them(self):
        self.publish()
        name = next(iter(asset_names(self.tag)))
        self.github.files[name] = b"damaged download"
        self.github.calls.clear()
        with self.assertRaisesRegex(SystemExit, "checksum mismatch"):
            self.publish()
        self.assertTrue(all(call[:2] == ("release", "download") for call in self.github.calls))
        del self.github.files[name]
        with self.assertRaisesRegex(SystemExit, "missing"):
            self.publish()

    def test_mismatched_upload_is_detected_before_publication(self):
        self.github.corrupt_upload = True
        with self.assertRaisesRegex(SystemExit, "checksum mismatch"):
            self.publish()
        self.assertTrue(self.github.release["draft"])

    def test_wrong_or_missing_architecture_assets_do_not_create_a_release(self):
        (self.directory / f"Caelum-{self.tag}-x86_64.dmg").unlink()
        with self.assertRaisesRegex(SystemExit, "exactly two"):
            self.publish()
        self.assertIsNone(self.github.release)

    def test_legacy_assets_without_github_digests_are_downloaded_and_hashed(self):
        self.publish()
        snapshot = self.github.snapshot()
        for asset in snapshot["assets"]:
            asset.pop("digest")
        self.github.calls.clear()
        verify_remote_assets("example/caelum", self.tag, snapshot)
        self.assertEqual(len(self.github.calls), 5)

    def test_prereleases_do_not_become_latest(self):
        previous = self.tag
        self.tag += "-rc.1"
        self.github.tag = self.tag
        for path in self.directory.iterdir():
            path.rename(path.with_name(path.name.replace(previous, self.tag)))
        self.publish()
        self.assertIn("--prerelease", self.github.calls[0])
        self.assertIn("prerelease=true", self.github.calls[-1])
        self.assertIn("make_latest=false", self.github.calls[-1])


class AssetTests(unittest.TestCase):
    def test_checksums_are_repeatable_and_do_not_include_the_manifest_itself(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            for name in asset_names("v1.2.3"):
                (directory / name).write_text(name)
            expected = prepare_assets(directory, "v1.2.3")
            self.assertEqual(prepare_assets(directory, "v1.2.3"), expected)
            text = (directory / "SHA256SUMS").read_text()
            self.assertEqual(parse_checksums(text, "v1.2.3"), expected)
            with self.assertRaises(SystemExit):
                parse_checksums(text + text.splitlines()[0], "v1.2.3")
            with self.assertRaises(SystemExit):
                parse_checksums(text.replace("Caelum-", "../Caelum-"), "v1.2.3")

    def test_tag_lookup_finds_drafts_by_release_id(self):
        draft = {"id": 17, "draft": True, "assets": []}
        with patch("publish_release.gh", side_effect=[
            json.dumps({"data": {"repository": {"release": {"databaseId": 17}}}}),
            json.dumps(draft),
        ]) as command:
            self.assertEqual(release_for_tag("example/caelum", "v1.2.3"), draft)
            command.assert_called_with("api", "repos/example/caelum/releases/17")

    def test_missing_release_is_a_successful_null_lookup(self):
        with patch("publish_release.gh", return_value=json.dumps({
            "data": {"repository": {"release": None}},
        })):
            self.assertIsNone(release_for_tag("example/caelum", "v1.2.3"))

    def test_api_errors_are_not_mistaken_for_a_missing_release(self):
        with patch("publish_release.gh", side_effect=subprocess.CalledProcessError(1, ["gh", "api"])):
            with self.assertRaises(subprocess.CalledProcessError):
                release_for_tag("example/caelum", "v1.2.3")

    def test_installation_notes_identify_architectures_and_the_no_apple_account_setup(self):
        notes = installation_notes("example/caelum", "v1.2.3")
        for name in asset_names("v1.2.3"):
            self.assertIn(name, notes)
        self.assertIn("ad-hoc signed and not notarized", notes)
        self.assertIn("Open Anyway", notes)
        self.assertIn("SHA256SUMS", notes)


if __name__ == "__main__":
    unittest.main()
