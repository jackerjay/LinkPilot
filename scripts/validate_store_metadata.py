#!/usr/bin/env python3
"""Validate the four-language App Store metadata before editing or submitting."""
import argparse
import json
import re
from pathlib import Path
from urllib.parse import urlparse

LOCALES = {"en-US": "en", "zh-Hans": "zh-CN", "zh-Hant": "zh-TW", "ja": "ja-JP"}
LIMITS = {"name": 30, "subtitle": 30, "promotional_text": 170,
          "description": 4000, "whats_new": 4000}


def jpeg_size(path):
    """Read JPEG frame dimensions without a platform imaging dependency."""
    with path.open("rb") as file:
        if file.read(2) != b"\xff\xd8":
            raise ValueError("not a JPEG")
        while file.read(1) == b"\xff":
            marker = file.read(1)
            while marker == b"\xff":
                marker = file.read(1)
            length = int.from_bytes(file.read(2), "big")
            if length < 2:
                break
            payload = file.read(length - 2)
            if marker in (b"\xc0", b"\xc1", b"\xc2") and len(payload) >= 5:
                return int.from_bytes(payload[3:5], "big"), int.from_bytes(payload[1:3], "big")
    raise ValueError("JPEG has no supported frame header")


def validate_screenshots(directory, locale, scenes):
    errors = []
    for scene in scenes:
        path = directory / "screenshots" / locale / f"{scene['id']}.jpg"
        try:
            if jpeg_size(path) != (1440, 900):
                errors.append(f"{path}: expected 1440x900 pixels")
        except (OSError, ValueError) as error:
            errors.append(f"{path}: {error}")
    return errors


def validate(data, locale):
    errors = []
    if data.get("locale") != locale:
        errors.append("locale does not match filename")
    for field, maximum in LIMITS.items():
        value = data.get(field)
        if not isinstance(value, str) or not value.strip():
            errors.append(f"{field} is required")
        elif len(value.encode("utf-16-le")) // 2 > maximum:
            errors.append(f"{field} exceeds {maximum} characters")
    keywords = data.get("keywords", "")
    if not isinstance(keywords, str) or not keywords:
        errors.append("keywords are required")
    elif len(keywords.encode("utf-8")) > 100:
        errors.append("keywords exceed Apple's 100-byte limit")
    else:
        terms = keywords.split(",")
        if any(not term or term != term.strip() for term in terms):
            errors.append("keywords contain empty terms or padding spaces")
        if len({term.casefold() for term in terms}) != len(terms):
            errors.append("keywords contain duplicate terms")
        if any(re.search(r"\b" + brand + r"\b", keywords, re.I) for brand in
               ["linkpilot", "chrome", "brave", "safari", "firefox", "openin", "velja", "bumpr"]):
            errors.append("keywords repeat a brand or another app/company name")
    for field in ("support_url", "marketing_url", "privacy_policy_url"):
        url = urlparse(data.get(field, ""))
        if url.scheme != "https" or not url.netloc:
            errors.append(f"{field} must be an HTTPS URL")
    scenes = data.get("screenshots", [])
    if [s.get("id") for s in scenes] != ["01-rules", "02-rule-editor", "03-test-url", "04-profiles"]:
        errors.append("the four screenshot scenes are missing or out of order")
    if any(not s.get("headline") or not s.get("caption") for s in scenes):
        errors.append("every screenshot needs localized headline and caption")
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", nargs="?", type=Path, default=Path("marketing/app-store/0.5.15"))
    args = parser.parse_args()
    errors = []
    for locale, ui_locale in LOCALES.items():
        path = args.directory / f"{locale}.json"
        try:
            data = json.loads(path.read_text())
        except (OSError, ValueError) as error:
            errors.append(f"{locale}: {error}")
            continue
        errors.extend(f"{locale}: {error}" for error in validate(data, locale))
        if data.get("version") != args.directory.name:
            errors.append(f"{locale}: metadata version differs from asset directory")
        errors.extend(validate_screenshots(args.directory, locale, data.get("screenshots", [])))
        pack = Path("apps/desktop/src/i18n/locales") / ui_locale / "app.json"
        if not pack.is_file():
            errors.append(f"{locale}: app does not ship the declared UI language")
        print(f"{locale}: name {len(data['name'])}/30, subtitle {len(data['subtitle'])}/30, keywords {len(data['keywords'].encode('utf-8'))}/100 bytes")
    if errors:
        raise SystemExit("\n".join(errors))
    print("Four-language metadata is valid")


if __name__ == "__main__":
    main()
