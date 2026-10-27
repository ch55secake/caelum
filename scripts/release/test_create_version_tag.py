import os
import tempfile
import unittest
from unittest.mock import patch

import create_version_tag as release_tag

VERSION_010 = {
    "Cargo workspace": "0.1.0",
    "Tauri app": "0.1.0",
    "frontend package": "0.1.0",
}
VERSION_020 = {
    "Cargo workspace": "0.2.0",
    "Tauri app": "0.2.0",
    "frontend package": "0.2.0",
}


class CreateVersionTagTests(unittest.TestCase):
    def setUp(self):
        self.output = tempfile.NamedTemporaryFile(mode="r+", delete=False)
        self.addCleanup(os.unlink, self.output.name)
        self.addCleanup(self.output.close)

    def run_main(self, versions, remote_commit=None):
        environment = {
            "GITHUB_OUTPUT": self.output.name,
            "GITHUB_REF": "refs/heads/main",
            "GITHUB_SHA": "new-commit",
            "BEFORE_SHA": "old-commit",
        }
        with (
            patch.dict(os.environ, environment),
            patch.object(release_tag, "project_versions", side_effect=versions),
            patch.object(
                release_tag,
                "workspace_lock_versions",
                return_value={"caelum-core": "0.2.0"},
            ),
            patch.object(release_tag, "remote_tag_commit", return_value=remote_commit),
            patch.object(release_tag.subprocess, "run") as run,
        ):
            release_tag.main()
        return run.call_args_list, self.output.read()

    def test_unchanged_version_skips_release(self):
        commands, output = self.run_main([VERSION_010, VERSION_010])

        self.assertEqual(commands, [])
        self.assertEqual(output, "release_tag=\nshould_release=false\n")

    def test_version_bump_creates_tag_at_merge_commit(self):
        commands, output = self.run_main([VERSION_020, VERSION_010])

        self.assertEqual(
            commands[0].args[0][-5:],
            ["-a", "v0.2.0", "new-commit", "-m", "Release v0.2.0"],
        )
        self.assertEqual(
            commands[1].args[0],
            ["git", "push", "origin", "refs/tags/v0.2.0"],
        )
        self.assertEqual(output, "release_tag=v0.2.0\nshould_release=true\n")

    def test_existing_tag_on_another_commit_is_not_moved(self):
        environment = {
            "GITHUB_OUTPUT": self.output.name,
            "GITHUB_REF": "refs/heads/main",
            "GITHUB_SHA": "new-commit",
            "BEFORE_SHA": "old-commit",
        }

        with (
            patch.dict(os.environ, environment),
            patch.object(
                release_tag,
                "project_versions",
                side_effect=[VERSION_020, VERSION_010],
            ),
            patch.object(
                release_tag,
                "workspace_lock_versions",
                return_value={"caelum-core": "0.2.0"},
            ),
            patch.object(release_tag, "remote_tag_commit", return_value="other-commit"),
            patch.object(release_tag.subprocess, "run") as run,
        ):
            with self.assertRaises(SystemExit):
                release_tag.main()

        run.assert_not_called()

    def test_stale_lockfile_blocks_tag_creation(self):
        environment = {
            "GITHUB_OUTPUT": self.output.name,
            "GITHUB_REF": "refs/heads/main",
            "GITHUB_SHA": "new-commit",
            "BEFORE_SHA": "old-commit",
        }

        with (
            patch.dict(os.environ, environment),
            patch.object(
                release_tag,
                "project_versions",
                side_effect=[VERSION_020, VERSION_010],
            ),
            patch.object(
                release_tag,
                "workspace_lock_versions",
                return_value={"caelum-core": "0.1.0"},
            ),
            patch.object(release_tag.subprocess, "run") as run,
        ):
            with self.assertRaises(SystemExit):
                release_tag.main()

        run.assert_not_called()

    def test_tag_push_reuses_the_existing_tag_trigger(self):
        environment = {
            "GITHUB_OUTPUT": self.output.name,
            "GITHUB_REF": "refs/tags/v0.2.0",
            "GITHUB_REF_NAME": "v0.2.0",
        }
        with patch.dict(os.environ, environment):
            release_tag.main()

        self.assertEqual(self.output.read(), "release_tag=v0.2.0\nshould_release=true\n")


if __name__ == "__main__":
    unittest.main()
