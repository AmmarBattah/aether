"""scripts/third_party_notices.py: parsing, rendering, the drift check, and one real cargo call."""
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import third_party_notices as notices  # noqa: E402

MIT = "MIT License\n\nPermission is hereby granted."
APACHE = "Apache License\nVersion 2.0"


def crate(name, version, licence, texts, repository="https://example.com/"):
    return {"name": name, "version": version, "licence": licence,
            "repository": repository + name if repository else "", "texts": texts}


class CargoTests(unittest.TestCase):
    def test_a_caller_that_forces_colour_still_gets_a_listing_that_parses(self):
        # The hosted CI exports CARGO_TERM_COLOR=always.
        with mock.patch.dict(os.environ, {"CARGO_TERM_COLOR": "always"}):
            listing = notices.cargo("tree", "--locked", "-p", notices.BINARY_PACKAGE, "-e", "normal",
                                    "--prefix", "none", "--format", "{p}")
        self.assertNotIn("\x1b", listing)
        self.assertIn("clap", {name for name, _ in notices.parse_tree(listing)})


class ParseTreeTests(unittest.TestCase):
    def test_only_third_party_crates_are_kept_once_each(self):
        listing = "\n".join([
            "aether v1.0.0 (/src/aether/crates/aether)",
            "serde v1.0.229",
            "serde_derive v1.0.229 (proc-macro)",
            "serde v1.0.229 (*)",
            "aether-core v1.0.0 (/src/aether/crates/aether-core) (*)",
            "hashbrown v0.15.5",
            "hashbrown v0.16.1",
            "",
        ])
        self.assertEqual(notices.parse_tree(listing), {
            ("serde", "1.0.229"), ("serde_derive", "1.0.229"),
            ("hashbrown", "0.15.5"), ("hashbrown", "0.16.1"),
        })

    def test_a_line_that_is_not_a_crate_is_an_error_not_skipped(self):
        with self.assertRaises(AssertionError):
            notices.parse_tree("warning: something changed in cargo tree")


class RenderTests(unittest.TestCase):
    def test_crates_are_sorted_and_a_shared_text_is_printed_once(self):
        rendered = notices.render([
            crate("zed", "2.0.0", "MIT", [("LICENSE", MIT)]),
            crate("alpha", "1.0.0", "MIT OR Apache-2.0",
                  [("LICENSE-APACHE", APACHE), ("LICENSE-MIT", MIT)]),
        ])
        self.assertTrue(rendered.startswith(notices.HEADER))
        self.assertLess(rendered.index("alpha 1.0.0: MIT OR Apache-2.0 (https://example.com/alpha)"),
                        rendered.index("zed 2.0.0: MIT (https://example.com/zed)"))
        self.assertEqual(rendered.count("Permission is hereby granted."), 1)
        self.assertEqual(rendered.count("Version 2.0"), 1)
        carriers = f"{notices.RULE}\nalpha 1.0.0 (LICENSE-MIT)\nzed 2.0.0 (LICENSE)\n{notices.RULE}"
        self.assertIn(carriers, rendered)
        self.assertTrue(rendered.endswith("Permission is hereby granted.\n"))

    def test_a_crate_without_a_licence_file_or_repository_is_still_listed(self):
        rendered = notices.render([crate("bare", "0.1.0", "MIT", [], repository="")])
        self.assertIn("bare 0.1.0: MIT\n    its package ships no licence file", rendered)

    def test_an_empty_or_repeated_crate_list_is_a_programming_error(self):
        with self.assertRaises(AssertionError):
            notices.render([])
        twice = [crate("alpha", "1.0.0", "MIT", [])] * 2
        with self.assertRaises(AssertionError):
            notices.render(twice)

    def test_text_is_normalised_so_line_ends_do_not_make_two_texts(self):
        self.assertEqual(notices.normalised("\r\nMIT  \r\n\r\nline\t\r\n\r\n"), "MIT\n\nline")


class DriftTests(unittest.TestCase):
    def test_a_missing_or_different_file_is_out_of_date(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "THIRD-PARTY-NOTICES"
            self.assertIn("does not exist", notices.out_of_date("expected\n", path))
            path.write_text("stale\n")
            self.assertIn("does not match Cargo.lock", notices.out_of_date("expected\n", path))
            path.write_text("expected\n")
            self.assertIsNone(notices.out_of_date("expected\n", path))


if __name__ == "__main__":
    unittest.main()
