#!/usr/bin/env python3
"""Validate a Store provisioning profile, sign a copy of the app, and create a pkg."""

import argparse
import datetime
import hashlib
import plistlib
import re
import shutil
import subprocess
from pathlib import Path


def distribution_entitlements(profile, bundle_id, certificate_sha1, now=None):
    now = now or datetime.datetime.now(datetime.timezone.utc)
    expiration = profile.get("ExpirationDate")
    if not isinstance(expiration, datetime.datetime):
        raise ValueError("Profile has no expiration date")
    if expiration.replace(tzinfo=datetime.timezone.utc) <= now:
        raise ValueError("Profile has expired")
    if profile.get("Platform") != ["OSX"]:
        raise ValueError("A macOS provisioning profile is required")
    entitlements = profile.get("Entitlements", {})
    if (profile.get("ProvisionedDevices") is not None
            or profile.get("ProvisionsAllDevices")
            or entitlements.get("get-task-allow")
            or entitlements.get("com.apple.security.get-task-allow")):
        raise ValueError("Use a Mac App Store Connect distribution profile")
    teams = profile.get("TeamIdentifier", [])
    if len(teams) != 1 or not re.fullmatch(r"[A-Z0-9]{10}", teams[0]):
        raise ValueError("Profile must identify exactly one Apple team")
    team = teams[0]
    app_id = entitlements.get("com.apple.application-identifier")
    if app_id != f"{team}.{bundle_id}":
        raise ValueError("Profile application identifier does not match the Store app")
    if entitlements.get("com.apple.developer.team-identifier") != team:
        raise ValueError("Profile entitlement team does not match its signing team")
    certificates = {hashlib.sha1(cert).hexdigest().upper()
                    for cert in profile.get("DeveloperCertificates", [])}
    if certificate_sha1.upper() not in certificates:
        raise ValueError("Application signing certificate is not included in the profile")
    return {
        "com.apple.application-identifier": app_id,
        "com.apple.developer.team-identifier": team,
        "com.apple.security.app-sandbox": True,
        "com.apple.security.network.client": True,
        "com.apple.security.files.user-selected.read-write": True,
    }


def run(*args, capture=False):
    return subprocess.run(args, check=True, capture_output=capture).stdout


def require_identity(fingerprint, prefixes, team, policy, keychain_args=()):
    if not re.fullmatch(r"[A-Fa-f0-9]{40}", fingerprint):
        raise ValueError("Choose a signing certificate by its 40-character SHA-1")
    identities = run("security", "find-identity", "-v", "-p", policy,
                     *keychain_args, capture=True).decode()
    names = re.findall(r'\b' + fingerprint + r'\s+"([^"]+)"', identities, re.I)
    if len(names) != 1 or not names[0].startswith(prefixes) or not names[0].endswith(f"({team})"):
        raise ValueError(f"No valid Store {policy} signing identity for team {team}: {fingerprint}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, required=True)
    parser.add_argument("--profile", type=Path, required=True)
    parser.add_argument("--application-identity", required=True)
    parser.add_argument("--installer-identity", required=True)
    parser.add_argument("--keychain", type=Path)
    parser.add_argument("--build-number", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not re.fullmatch(r"[1-9][0-9]*", args.build_number):
        parser.error("build-number must be a positive integer, incremented for every upload")
    info = plistlib.loads((args.app / "Contents/Info.plist").read_bytes())
    if info.get("CFBundleIdentifier") != "app.linkpilot.store":
        parser.error("Only the Store bundle may be packaged")
    for binary in ("lpt", "linkpilot-daemon", "linkpilot-native-host"):
        if (args.app / "Contents/MacOS" / binary).exists():
            parser.error(f"Store bundle contains forbidden helper: {binary}")
    privacy = args.app / "Contents/Resources/PrivacyInfo.xcprivacy"
    plistlib.loads(privacy.read_bytes())
    profile = plistlib.loads(run("security", "cms", "-D", "-i", str(args.profile), capture=True))
    entitlements = distribution_entitlements(
        profile, info["CFBundleIdentifier"], args.application_identity)
    team = entitlements["com.apple.developer.team-identifier"]
    keychain_path = (str(args.keychain),) if args.keychain else ()
    keychain_flag = ("--keychain", str(args.keychain)) if args.keychain else ()
    require_identity(args.application_identity,
                     ("3rd Party Mac Developer Application:", "Apple Distribution:"),
                     team, "codesigning", keychain_path)
    require_identity(args.installer_identity,
                     ("3rd Party Mac Developer Installer:",), team, "basic", keychain_path)
    # Never alter the tested development bundle or overwrite a previous submission.
    args.output.mkdir(parents=True, exist_ok=False)
    app = args.output / args.app.name
    shutil.copytree(args.app, app, symlinks=True)
    shutil.copyfile(args.profile, app / "Contents/embedded.provisionprofile")
    info["CFBundleVersion"] = args.build_number
    (app / "Contents/Info.plist").write_bytes(plistlib.dumps(info))
    entitlements_path = args.output / "Entitlements.plist"
    entitlements_path.write_bytes(plistlib.dumps(entitlements))
    run("codesign", "--force", "--sign", args.application_identity, "--timestamp",
        *keychain_flag, "--options", "runtime", "--entitlements", str(entitlements_path), str(app))
    run("codesign", "--verify", "--deep", "--strict", "--verbose=2", str(app))
    signed = plistlib.loads(run("codesign", "-d", "--entitlements", ":-", str(app), capture=True))
    if signed != entitlements:
        raise ValueError("Final code-signing entitlements differ from validated entitlements")
    pkg = args.output / f"LinkPilot_{info['CFBundleShortVersionString']}_{args.build_number}.pkg"
    run("xcrun", "productbuild", "--sign", args.installer_identity,
        *keychain_flag, "--component", str(app), "/Applications", str(pkg))
    run("pkgutil", "--check-signature", str(pkg))
    digest = hashlib.sha256(pkg.read_bytes()).hexdigest()
    (args.output / "checksums.txt").write_text(f"{digest}  {pkg.name}\n")
    print(f"Signed Store package (not yet uploaded): {pkg}")


if __name__ == "__main__":
    main()
