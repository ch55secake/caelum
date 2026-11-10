import json
import unittest

from close_superseded_homebrew_prs import close_superseded_prs, version


class CloseSupersededHomebrewPullRequestsTests(unittest.TestCase):
    def test_version_requires_a_stable_semver_tag(self):
        self.assertEqual(version("v0.1.45"), (0, 1, 45))
        self.assertEqual(version("1.2.3"), (1, 2, 3))
        for value in ("latest", "v0.1.45-rc.1", "v1.2"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                version(value)

    def test_closes_only_older_caelum_update_prs_and_deletes_their_branches(self):
        calls = []
        pull_requests = [
            {"number": 12, "headRefName": "automation/caelum-v0.1.44"},
            {"number": 13, "headRefName": "automation/caelum-v0.1.45"},
            {"number": 14, "headRefName": "automation/caelum-v0.1.46"},
            {"number": 15, "headRefName": "automation/caelum-v0.1.44-rc.1"},
            {"number": 16, "headRefName": "feature/unrelated"},
        ]

        def run_gh(*args):
            calls.append(args)
            return json.dumps(pull_requests) if args[:2] == ("pr", "list") else ""

        self.assertEqual(close_superseded_prs("v0.1.45", run_gh), [12])
        self.assertEqual(
            calls[1],
            (
                "pr",
                "close",
                "12",
                "--repo",
                "ch55secake/homebrew-tap",
                "--delete-branch",
                "--comment",
                "Superseded by Caelum v0.1.45.",
            ),
        )

    def test_invalid_release_tag_does_not_query_or_close_pull_requests(self):
        calls = []
        with self.assertRaises(ValueError):
            close_superseded_prs("latest", lambda *args: calls.append(args) or "[]")
        self.assertEqual(calls, [])


if __name__ == "__main__":
    unittest.main()
