"""The signing team must come from the certificate OU, never the person ID."""
import hashlib
import importlib.util
import unittest
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("signing_team", Path(__file__).with_name("signing-team.py"))
signing = importlib.util.module_from_spec(spec)
spec.loader.exec_module(signing)


class SigningTeamTests(unittest.TestCase):
    def test_development_person_suffix_is_not_the_team(self):
        certificate = b"test certificate DER"
        fingerprint = hashlib.sha1(certificate).hexdigest().upper()
        identities = f'1) {fingerprint} "Apple Development: Developer (PERSON0001)"'
        for subject in ["subject=OU=TEAMID0001,CN=Developer\n", "subject=CN=Developer,OU=TEAMID0001,C=US\n"]:
            with self.subTest(subject=subject), patch.object(signing.subprocess, "check_output", side_effect=[identities,
                    "-----BEGIN CERTIFICATE-----\nfixture\n-----END CERTIFICATE-----", subject]), \
                    patch.object(signing.ssl, "PEM_cert_to_DER_cert", return_value=certificate):
                self.assertEqual(signing.signing_team(fingerprint), "TEAMID0001")

    def test_unknown_identity_does_not_pick_another_certificate(self):
        with patch.object(signing.subprocess, "check_output", return_value="no valid identities"):
            with self.assertRaises(ValueError):
                signing.signing_team("0" * 40)


if __name__ == "__main__":
    unittest.main()
