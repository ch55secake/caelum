import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from create_version_tag import (
    bump_patch_version,
    tag_push_args,
    update_version_files,
    workspace_lock_versions_from_text,
)


class VersionBumpTests(unittest.TestCase):
    def test_patch_bump_increments_numeric_patch_component(self):
        self.assertEqual(bump_patch_version("0.1.1"), "0.1.2")
        self.assertEqual(bump_patch_version("1.4.9"), "1.4.10")

    def test_patch_bump_rejects_prerelease_versions(self):
        with self.assertRaises(SystemExit):
            bump_patch_version("1.2.3-rc.1")

    def test_tag_push_resets_checkout_auth_header_before_using_actions_token(self):
        args = tag_push_args("v0.1.2", "actions-token")

        self.assertEqual(args[1:3], ["-c", "http.https://github.com/.extraheader="])
        self.assertEqual(args[3], "-c")
        self.assertTrue(
            args[4].startswith("http.https://github.com/.extraheader=AUTHORIZATION: basic ")
        )
        self.assertEqual(args[-3:], ["push", "origin", "refs/tags/v0.1.2"])

    def test_updates_project_manifests_and_only_local_lock_packages(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            (root / "apps/desktop/src-tauri").mkdir(parents=True)
            (root / "Cargo.toml").write_text(
                "[workspace.package]\nversion = \"0.1.9\"\n", encoding="utf-8"
            )
            (root / "apps/desktop/package.json").write_text(
                '{\n  "name": "@caelum/desktop",\n  "version": "0.1.9"\n}\n',
                encoding="utf-8",
            )
            (root / "apps/desktop/src-tauri/tauri.conf.json").write_text(
                '{\n  "productName": "Caelum",\n  "version": "0.1.9"\n}\n',
                encoding="utf-8",
            )
            (root / "Cargo.lock").write_text(
                """[[package]]
name = "caelum-cli"
version = "0.1.9"

[[package]]
name = "caelum-core"
version = "0.1.9"

[[package]]
name = "caelum-helper"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
""",
                encoding="utf-8",
            )

            update_version_files(root, "0.1.10")

            self.assertIn('version = "0.1.10"', (root / "Cargo.toml").read_text())
            package = json.loads((root / "apps/desktop/package.json").read_text())
            tauri = json.loads((root / "apps/desktop/src-tauri/tauri.conf.json").read_text())
            self.assertEqual(package["version"], "0.1.10")
            self.assertEqual(tauri["version"], "0.1.10")
            lockfile = (root / "Cargo.lock").read_text()
            self.assertEqual(
                workspace_lock_versions_from_text(lockfile),
                {"caelum-cli": "0.1.10", "caelum-core": "0.1.10"},
            )
            self.assertIn('name = "caelum-helper"\nversion = "0.1.0"', lockfile)


class AutomaticReleaseTests(unittest.TestCase):
    def git(self, repository: Path, *args: str) -> str:
        return subprocess.check_output(
            ["git", *args], cwd=repository, text=True
        ).strip()

    def test_main_merge_creates_patch_commit_and_rerun_reuses_it(self):
        script = Path(__file__).with_name("create_version_tag.py")
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            repository = root / "repository"
            remote = root / "remote.git"
            repository.mkdir()
            subprocess.run(["git", "init", "--quiet", str(repository)], check=True)
            self.git(repository, "branch", "-M", "main")
            self.git(repository, "config", "user.name", "Release test")
            self.git(repository, "config", "user.email", "release-test@example.com")
            self.git(repository, "config", "commit.gpgsign", "false")
            self.git(repository, "config", "tag.gpgsign", "false")
            subprocess.run(["git", "init", "--bare", "--quiet", str(remote)], check=True)

            (repository / "apps/desktop/src-tauri").mkdir(parents=True)
            (repository / "apps/desktop/package.json").write_text(
                '{"name":"@caelum/desktop","version":"0.1.1"}\n', encoding="utf-8"
            )
            (repository / "apps/desktop/src-tauri/tauri.conf.json").write_text(
                '{"productName":"Caelum","version":"0.1.1"}\n', encoding="utf-8"
            )
            (repository / "Cargo.toml").write_text(
                "[workspace.package]\nversion = \"0.1.1\"\n", encoding="utf-8"
            )
            (repository / "Cargo.lock").write_text(
                '[[package]]\nname = "caelum-cli"\nversion = "0.1.1"\n\n'
                '[[package]]\nname = "caelum-core"\nversion = "0.1.1"\n',
                encoding="utf-8",
            )
            (repository / "README.md").write_text("release test\n", encoding="utf-8")
            self.git(repository, "add", ".")
            self.git(repository, "commit", "-m", "chore(release): initial version")
            base_commit = self.git(repository, "rev-parse", "HEAD")
            self.git(repository, "remote", "add", "origin", str(remote))
            self.git(repository, "push", "--quiet", "-u", "origin", "main")

            (repository / "README.md").write_text("merged feature\n", encoding="utf-8")
            self.git(repository, "add", "README.md")
            self.git(repository, "commit", "-m", "feat: add merged feature")
            source_commit = self.git(repository, "rev-parse", "HEAD")
            self.git(repository, "push", "--quiet", "origin", "main")

            output_path = root / "github-output"
            environment = os.environ.copy()
            environment.update(
                {
                    "GITHUB_REF": "refs/heads/main",
                    "GITHUB_EVENT_NAME": "push",
                    "GITHUB_SHA": source_commit,
                    "BEFORE_SHA": base_commit,
                    "GITHUB_OUTPUT": str(output_path),
                }
            )
            subprocess.run([sys.executable, str(script)], cwd=repository, env=environment, check=True)

            outputs = dict(
                line.split("=", maxsplit=1)
                for line in output_path.read_text(encoding="utf-8").splitlines()
            )
            self.assertEqual(outputs["release_tag"], "v0.1.2")
            self.assertEqual(outputs["should_release"], "true")
            release_commit = outputs["release_commit"]
            self.git(repository, "fetch", "--quiet", "origin", "main")
            self.assertEqual(self.git(repository, "rev-parse", "origin/main"), release_commit)
            self.assertEqual(self.git(repository, "rev-parse", "v0.1.2^{}"), release_commit)
            self.assertIn('version = "0.1.2"', self.git(repository, "show", "origin/main:Cargo.toml"))

            self.git(repository, "checkout", "--quiet", "--detach", source_commit)
            retry_output = root / "retry-output"
            environment["GITHUB_OUTPUT"] = str(retry_output)
            subprocess.run([sys.executable, str(script)], cwd=repository, env=environment, check=True)
            retry_outputs = dict(
                line.split("=", maxsplit=1)
                for line in retry_output.read_text(encoding="utf-8").splitlines()
            )
            self.assertEqual(retry_outputs["release_commit"], release_commit)
            self.assertEqual(retry_outputs["release_tag"], "v0.1.2")

            bot_push_output = root / "bot-push-output"
            environment.update(
                {
                    "GITHUB_SHA": release_commit,
                    "BEFORE_SHA": source_commit,
                    "GITHUB_OUTPUT": str(bot_push_output),
                }
            )
            subprocess.run([sys.executable, str(script)], cwd=repository, env=environment, check=True)
            bot_push_outputs = dict(
                line.split("=", maxsplit=1)
                for line in bot_push_output.read_text(encoding="utf-8").splitlines()
            )
            self.assertEqual(bot_push_outputs["release_tag"], "")
            self.assertEqual(bot_push_outputs["should_release"], "false")

            # Manual recovery resolves the original tag, not the checkout's HEAD,
            # and never makes another version commit.
            recovery_output = root / "recovery-output"
            environment.update({
                "GITHUB_EVENT_NAME": "workflow_dispatch",
                "RELEASE_TAG": "v0.1.2",
                "GITHUB_OUTPUT": str(recovery_output),
            })
            subprocess.run([sys.executable, str(script)], cwd=repository, env=environment, check=True)
            self.assertIn(f"release_commit={release_commit}", recovery_output.read_text())
            self.assertEqual(self.git(repository, "ls-remote", "origin", "refs/heads/main").split()[0], release_commit)

            for tag in ["v9.9.9", "v0.1.99", "--all"]:
                if tag == "v9.9.9":
                    self.git(repository, "tag", tag, release_commit)
                    self.git(repository, "push", "origin", f"refs/tags/{tag}")
                environment["RELEASE_TAG"] = tag
                rejected = subprocess.run([sys.executable, str(script)], cwd=repository, env=environment, capture_output=True, text=True)
                self.assertNotEqual(rejected.returncode, 0, tag)

            # An older queued merge must not silently release a newer tree.
            self.git(repository, "checkout", "--detach", release_commit)
            (repository / "README.md").write_text("first overlapping merge\n")
            self.git(repository, "commit", "-am", "feat: first overlapping merge")
            first = self.git(repository, "rev-parse", "HEAD")
            self.git(repository, "push", "origin", "HEAD:main")
            (repository / "README.md").write_text("second overlapping merge\n")
            self.git(repository, "commit", "-am", "feat: second overlapping merge")
            second = self.git(repository, "rev-parse", "HEAD")
            self.git(repository, "push", "origin", "HEAD:main")
            self.git(repository, "checkout", "--detach", first)
            superseded_output = root / "superseded-output"
            environment.update({
                "GITHUB_EVENT_NAME": "push", "GITHUB_SHA": first,
                "BEFORE_SHA": release_commit, "GITHUB_OUTPUT": str(superseded_output),
            })
            subprocess.run([sys.executable, str(script)], cwd=repository, env=environment, check=True)
            self.assertIn("should_release=false", superseded_output.read_text())
            self.assertEqual(self.git(repository, "ls-remote", "origin", "refs/heads/main").split()[0], second)
            self.assertEqual(self.git(repository, "ls-remote", "origin", "refs/tags/v0.1.3"), "")

            self.git(repository, "checkout", "--detach", second)
            environment.update({"GITHUB_SHA": second, "BEFORE_SHA": first, "GITHUB_OUTPUT": str(root / "latest-output")})
            subprocess.run([sys.executable, str(script)], cwd=repository, env=environment, check=True)
            self.assertEqual(self.git(repository, "rev-parse", "v0.1.3^"), second)
            self.assertEqual(self.git(repository, "show", "v0.1.3:README.md"), "second overlapping merge")


if __name__ == "__main__":
    unittest.main()
