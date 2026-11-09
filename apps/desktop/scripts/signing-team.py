#!/usr/bin/env python3
"""Resolve one valid codesigning identity to its Apple team, without exporting keys."""
import re
import subprocess
import sys
import ssl
import hashlib


def signing_team(identity):
    output = subprocess.check_output(["security", "find-identity", "-v", "-p", "codesigning"], text=True)
    matches = re.findall(r'\b([A-F0-9]{40})\s+"([^"\n]+) \(([A-Z0-9]{10})\)"', output)
    fingerprints = {fingerprint for fingerprint, name, suffix in matches if identity in (fingerprint, f"{name} ({suffix})")}
    if len(fingerprints) != 1:
        raise ValueError("Choose exactly one valid Apple codesigning identity")
    fingerprint = fingerprints.pop()
    certificates = subprocess.check_output(["security", "find-certificate", "-a", "-p"], text=True)
    for pem in re.findall(r"-----BEGIN CERTIFICATE-----.*?-----END CERTIFICATE-----", certificates, re.S):
        if hashlib.sha1(ssl.PEM_cert_to_DER_cert(pem)).hexdigest().upper() == fingerprint:
            subject = subprocess.check_output(["openssl", "x509", "-noout", "-subject", "-nameopt", "RFC2253"], input=pem, text=True)
            # Apple Development's common-name suffix is the person ID, not the
            # signing team. The certificate subject OU is authoritative.
            team = re.search(r"(?:^|,)OU=([A-Z0-9]{10})(?:,|$)", subject.strip().removeprefix("subject="))
            if team:
                return team.group(1)
    raise ValueError("Signing certificate has no valid Apple team identifier")


if __name__ == "__main__":
    print(signing_team(sys.argv[1]))
