"""Guard byte limits and missing localizations instead of relying on UI counters."""
import copy
import json
import tempfile
import unittest
from pathlib import Path
from validate_store_metadata import jpeg_size, validate, validate_screenshots


class MetadataTests(unittest.TestCase):
    def setUp(self):
        self.data = json.loads(Path("marketing/app-store/0.5.15/en-US.json").read_text())

    def test_valid_source(self):
        self.assertEqual(validate(self.data, "en-US"), [])

    def test_cjk_keyword_byte_limit_is_stricter_than_character_count(self):
        data = copy.deepcopy(self.data)
        data["keywords"] = "路由" * 20
        self.assertLess(len(data["keywords"]), 100)
        self.assertIn("keywords exceed Apple's 100-byte limit", validate(data, "en-US"))

    def test_overlong_title_and_wrong_locale_fail(self):
        data = copy.deepcopy(self.data)
        data["name"] = "x" * 31
        self.assertIn("name exceeds 30 characters", validate(data, "zh-Hans"))
        self.assertIn("locale does not match filename", validate(data, "zh-Hans"))

    def test_duplicate_or_competitor_keywords_fail(self):
        for keywords in ("opener,Opener", "opener,OpenIn"):
            data = copy.deepcopy(self.data)
            data["keywords"] = keywords
            self.assertTrue(validate(data, "en-US"))

    def test_missing_or_clipped_screenshots_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.assertEqual(len(validate_screenshots(root, "en-US", self.data["screenshots"])), 4)
            path = root / "screenshots/en-US/01-rules.jpg"
            path.parent.mkdir(parents=True)
            # A valid frame header with the accidentally clipped viewport height.
            path.write_bytes(b"\xff\xd8\xff\xc0\x00\x07\x08\x02\x02\x05\xa0")
            self.assertEqual(jpeg_size(path), (1440, 514))
            self.assertIn("expected 1440x900 pixels", validate_screenshots(root, "en-US", self.data["screenshots"])[0])
            path.write_bytes(b"not an image")
            self.assertIn("not a JPEG", validate_screenshots(root, "en-US", self.data["screenshots"])[0])


if __name__ == "__main__":
    unittest.main()
