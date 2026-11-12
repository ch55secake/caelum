import unittest

from check import diagnostics, folder_sizes, parse_exceptions, ratchet, tighten
from rust import measure


class RustCountingTests(unittest.TestCase):
    def test_inline_tests_ignore_literals_comments_lifetimes_and_nested_modules(self):
        source = '''fn production<'a>(v: &'a str) {}
#[cfg(test)]
mod tests {
    const RAW: &str = r###"}
#[cfg(test)]
mod fake {"###;
    /* } /* { */ } */
    const CHAR: char = '}';
    const ESCAPE: char = '\\u{7d}';
    const STRING: &str = "} \\" {";
    #[cfg(test)]
    mod nested { fn helper() {} }
}
fn after() {}
'''
        size = measure("crates/example/src/lib.rs", source)
        self.assertEqual(size.source, 2)
        self.assertEqual(size.tests, 12)

    def test_attributes_and_external_test_modules_exclude_only_the_test_item(self):
        source = '''#[cfg(test)]
#[path = "suite.rs"]
mod tests;
pub fn production() {}
#[cfg(test)]
fn helper() { let _ = br#"} {"#; }
pub fn more() {}
'''
        size = measure("apps/example/src/main.rs", source)
        self.assertEqual((size.source, size.tests), (2, 3))

    def test_test_paths_and_individual_inline_module_limits(self):
        for path in ["apps/a/src/tests.rs", "crates/a/src/request_tests.rs", "apps/a/tests/helpers.rs"]:
            self.assertEqual(measure(path, "a\nb\n").tests, 2)
            self.assertEqual(measure(path, "a\nb\n").source, 0)
        size = measure("crates/a/src/lib.rs", "#[cfg(test)]\nmod a {}\n#[cfg(test)]\nmod b {}\n")
        self.assertEqual((size.source, size.tests), (0, 2))

    def test_cfg_text_in_a_production_raw_string_does_not_hide_source(self):
        source = 'const FIXTURE: &str = r#"\n#[cfg(test)]\nmod hidden {}\n"#;\nfn next() {}\n'
        self.assertEqual((measure("lib.rs", source).source, measure("lib.rs", source).tests), (5, 0))

    def test_platform_gated_tests_are_excluded_but_optional_production_code_is_not(self):
        source = '''#[cfg(all(test, unix))]
mod tests { fn helper() {} }
#[cfg(any(test, feature = "production"))]
fn production() {}
#[cfg(not(test))]
fn release_only() {}
'''
        size = measure("lib.rs", source)
        self.assertEqual((size.source, size.tests), (4, 2))


class GuardrailTests(unittest.TestCase):
    def test_source_and_test_caps_and_existing_ceilings(self):
        sizes = {("source", "large.rs"): 1501, ("source", "medium.rs"): 801, ("tests", "tests.rs"): 1001}
        self.assertEqual([row[0] for row in diagnostics(sizes, {})], ["error", "error", "error"])
        exceptions = {("source", "large.rs"): 1501, ("tests", "tests.rs"): 1001}
        self.assertEqual(
            [row[0] for row in diagnostics(sizes, exceptions)],
            ["warning", "error", "warning"],
        )
        sizes[("source", "large.rs")] += 1
        self.assertEqual(diagnostics(sizes, exceptions)[0][0], "error")

    def test_over_800_line_sources_block_changes_but_allow_legacy_shrinks(self):
        legacy = ("source", "legacy.rs")
        sizes = {legacy: 900}
        baseline = {"legacy.rs": 900}
        self.assertEqual(diagnostics(sizes, {}, baseline, set())[0][0], "warning")
        self.assertEqual(diagnostics(sizes, {}, baseline, {"legacy.rs"})[0][0], "error")
        self.assertEqual(diagnostics({legacy: 850}, {}, baseline, {"legacy.rs"})[0][0], "warning")
        self.assertEqual(diagnostics({legacy: 901}, {}, baseline, {"legacy.rs"})[0][0], "error")
        self.assertEqual(diagnostics({legacy: 801}, {}, {}, {"legacy.rs"})[0][0], "error")

    def test_ratchet_refuses_new_and_raised_exceptions(self):
        old = parse_exceptions("# baseline\nsource big.rs 2000\ntests tests.rs 1500\n")
        self.assertEqual(ratchet(old, {("source", "big.rs"): 1800}), [])
        self.assertEqual(len(ratchet(old, {("source", "big.rs"): 2001})), 1)
        self.assertEqual(len(ratchet(old, {("source", "new.rs"): 1600})), 1)
        self.assertEqual(tighten(old, {("source", "big.rs"): 1800, ("tests", "tests.rs"): 900}),
                         {("source", "big.rs"): 1800})
        with self.assertRaises(ValueError):
            parse_exceptions("source a.rs 1600\nsource a.rs 1700\n")

    def test_capability_families_and_folder_breadth_exclude_tests_and_binaries(self):
        paths = ["crates/a/src/review.rs", "crates/a/src/review_files.rs", "crates/a/src/review_threads.rs",
                 "crates/a/src/review_tests.rs", "crates/a/src/bin/review_run.rs"]
        self.assertEqual(folder_sizes(paths), {("family", "crates/a/src/review_*"): 2})
        paths = ["apps/a/src/editor/mod.rs", "apps/a/src/editor_input.rs", "apps/a/src/editor_scroll.rs"]
        self.assertIn(("family", "apps/a/src/editor_*"), folder_sizes(paths))
        paths = [f"crates/a/src/file{i}.rs" for i in range(61)]
        self.assertEqual(folder_sizes(paths), {("folder", "crates/a/src"): 61})
        self.assertEqual(diagnostics(folder_sizes(paths), {})[0][0], "error")


if __name__ == "__main__":
    unittest.main()
