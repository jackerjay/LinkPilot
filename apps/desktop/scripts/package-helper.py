#!/usr/bin/env python3
"""Package the optional native Helper separately from the sandboxed Store app."""
import argparse
import plistlib
import shutil
import subprocess
from pathlib import Path
from importlib.util import spec_from_file_location, module_from_spec

spec = spec_from_file_location("signing_team", Path(__file__).with_name("signing-team.py"))
signing = module_from_spec(spec)
spec.loader.exec_module(signing)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--identity", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--development", action="store_true")
    args = parser.parse_args()
    team = signing.signing_team(args.identity)
    app = args.output / "LinkPilot Helper.app"
    app.mkdir(parents=True, exist_ok=False)
    contents = app / "Contents"
    (contents / "MacOS").mkdir(parents=True)
    (contents / "Resources").mkdir()
    binary = contents / "MacOS/linkpilot-helper"
    shutil.copy2(args.binary, binary)
    binary.chmod(0o755)
    icon = Path(__file__).resolve().parents[1] / "src-tauri/icons/icon.icns"
    shutil.copy2(icon, contents / "Resources/icon.icns")
    (contents / "Info.plist").write_bytes(plistlib.dumps({
        "CFBundleIdentifier": "app.linkpilot.helper",
        "CFBundleName": "LinkPilot Helper",
        "CFBundleExecutable": "linkpilot-helper",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": args.version,
        "CFBundleVersion": args.version,
        "CFBundleIconFile": "icon.icns",
        "LSMinimumSystemVersion": "12.0",
        "LSUIElement": True,
        "NSHighResolutionCapable": True,
    }))
    entitlements = args.output / "Helper.entitlements.plist"
    entitlements.write_bytes(plistlib.dumps({
        "com.apple.security.application-groups": [f"{team}.app.linkpilot"],
    }))
    timestamp = "--timestamp=none" if args.development else "--timestamp"
    subprocess.run(["codesign", "--force", "--sign", args.identity, "--options", "runtime",
                    timestamp, "--entitlements", str(entitlements), str(app)], check=True)
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(app)], check=True)
    print(app)


if __name__ == "__main__":
    main()
