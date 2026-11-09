#!/usr/bin/env python3
"""Test actual App Sandbox and XPC peer requirements using Apple-signed code.

Registers a temporary, per-user Mach service and removes it on exit. Does not
install a LaunchAgent, open browsers, read profiles, or modify production config.
"""
import argparse
import json
import os
import plistlib
import re
import shutil
import subprocess
import tempfile
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--helper", type=Path, required=True)
    parser.add_argument("--identity", required=True)
    args = parser.parse_args()
    helper = args.helper.resolve() / "Contents/MacOS/linkpilot-helper"
    signature = subprocess.run(["codesign", "-dv", str(helper)], capture_output=True, text=True, check=True)
    team = re.search(r"TeamIdentifier=([A-Z0-9]{10})", signature.stderr).group(1)
    group = f"{team}.app.linkpilot"
    label = f"{group}.helper"
    domain = f"gui/{os.geteuid()}"
    target = f"{domain}/{label}"

    def ctl(*params):
        return subprocess.run(["/bin/launchctl", *params], capture_output=True, text=True, timeout=15)

    def run(binary, *params):
        return subprocess.run([str(binary), *params], capture_output=True, text=True, timeout=15)

    if ctl("print", target).returncode == 0:
        parser.error("Helper is already registered. Disable it before this isolated test.")
    proofs = []
    with tempfile.TemporaryDirectory(prefix="linkpilot-helper-test-") as folder:
        root = Path(folder)

        def client(name, identifier, sandbox=True):
            app = root / f"{name}.app"
            contents = app / "Contents"
            (contents / "MacOS").mkdir(parents=True)
            binary = contents / "MacOS/linkpilot-helper"
            shutil.copy2(helper, binary)
            (contents / "Info.plist").write_bytes(plistlib.dumps({
                "CFBundleIdentifier": identifier, "CFBundleExecutable": "linkpilot-helper", "CFBundlePackageType": "APPL"}))
            ent = root / f"{name}.plist"
            values = {"com.apple.security.application-groups": [group]}
            if sandbox:
                values.update({"com.apple.security.app-sandbox": True, "com.apple.security.network.client": True})
            ent.write_bytes(plistlib.dumps(values))
            subprocess.run(["codesign", "--force", "--sign", args.identity,
                "--identifier", identifier, "--timestamp=none", "--options", "runtime",
                "--entitlements", str(ent), str(app)], check=True, capture_output=True)
            return binary

        sandboxed = client("sandbox-client", "app.linkpilot.store")
        untrusted = client("untrusted-client", "app.linkpilot.untrusted-test")
        fake = client("fake-helper", "app.linkpilot.untrusted-test", False)

        def bootstrap(binary):
            agent = root / "agent.plist"
            agent.write_bytes(plistlib.dumps({"Label": label,
                "ProgramArguments": [str(binary), "serve"], "MachServices": {label: True},
                "StandardErrorPath": str(root / "service.log")}))
            result = ctl("bootstrap", domain, str(agent))
            assert result.returncode == 0, result.stderr
            return agent

        try:
            agent = bootstrap(helper)
            result = run(sandboxed, "status")
            assert result.returncode == 0, f"sandbox client exited {result.returncode}: {result.stderr}"
            assert json.loads(result.stdout)["kind"] == "status"
            proofs.append("signed sandbox client communicates with signed Helper")
            assert run(untrusted, "status").returncode != 0
            proofs.append("same-team client with wrong signing identifier is rejected")
            assert ctl("bootstrap", domain, str(agent)).returncode != 0
            assert run(sandboxed, "status").returncode == 0
            proofs.append("duplicate registration cannot replace live service")
            assert ctl("bootout", target).returncode == 0
            assert run(sandboxed, "status").returncode != 0
            proofs.append("missing Helper fails visibly")
            bootstrap(fake)
            result = run(sandboxed, "status")
            assert result.returncode != 0, result.stdout
            proofs.append("client rejects replacement service with untrusted signing identifier")
        finally:
            ctl("bootout", target)
        print(json.dumps({"passed": proofs}, indent=2))


if __name__ == "__main__":
    main()
