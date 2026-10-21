"""Reject mismatched/development profiles before modifying an app bundle."""

import copy
import datetime
import hashlib
import importlib.util
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "package_store", Path(__file__).with_name("package-store.py"))
package_store = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package_store)


class StoreProfileTests(unittest.TestCase):
    def setUp(self):
        self.now = datetime.datetime(2026, 10, 21, tzinfo=datetime.timezone.utc)
        self.fingerprint = hashlib.sha1(b"distribution certificate").hexdigest().upper()
        self.profile = {
            "ExpirationDate": datetime.datetime(2027, 10, 21),
            "Platform": ["OSX"],
            "TeamIdentifier": ["348D74H42G"],
            "DeveloperCertificates": [b"distribution certificate"],
            "Entitlements": {
                "com.apple.application-identifier": "348D74H42G.app.linkpilot.store",
                "com.apple.developer.team-identifier": "348D74H42G",
            },
        }

    def validate(self, profile=None, fingerprint=None):
        return package_store.distribution_entitlements(
            self.profile if profile is None else profile, "app.linkpilot.store",
            self.fingerprint if fingerprint is None else fingerprint, self.now)

    def test_approved_profile_keeps_only_required_entitlements(self):
        self.profile["Entitlements"]["com.apple.security.get-task-allow"] = False
        result = self.validate()
        self.assertTrue(result["com.apple.security.app-sandbox"])
        self.assertEqual(result["com.apple.application-identifier"],
                         "348D74H42G.app.linkpilot.store")
        self.assertEqual(len(result), 5)

    def test_rejects_expired_profile(self):
        self.profile["ExpirationDate"] = datetime.datetime(2026, 10, 20)
        with self.assertRaisesRegex(ValueError, "expired"):
            self.validate()

    def test_rejects_other_bundle_or_team(self):
        for key, value in (("com.apple.application-identifier", "348D74H42G.app.other"),
                           ("com.apple.developer.team-identifier", "OTHERTEAM1")):
            profile = copy.deepcopy(self.profile)
            profile["Entitlements"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.validate(profile)

    def test_rejects_wrong_signing_certificate(self):
        with self.assertRaisesRegex(ValueError, "not included"):
            self.validate(fingerprint="0" * 40)

    def test_rejects_development_and_developer_id_profiles(self):
        for key, value in (("ProvisionedDevices", ["test-device"]),
                           ("ProvisionsAllDevices", True)):
            profile = copy.deepcopy(self.profile)
            profile[key] = value
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "distribution profile"):
                self.validate(profile)
        self.profile["Entitlements"]["com.apple.security.get-task-allow"] = True
        with self.assertRaisesRegex(ValueError, "distribution profile"):
            self.validate()

    def test_rejects_non_macos_profile(self):
        self.profile["Platform"] = ["iOS"]
        with self.assertRaisesRegex(ValueError, "macOS"):
            self.validate()


if __name__ == "__main__":
    unittest.main()
