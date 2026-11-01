import { test } from "node:test";
import assert from "node:assert/strict";
import { createUpdateRunner } from "../src/lib/update-flow.ts";

function fixture(overrides = {}) {
  const states = [];
  const installs = [];
  let downloads = 0;
  const metadata = {
    latestVersion: "0.5.12", available: true, installError: null,
    asset: { name: "update.dmg", sha256: "a".repeat(64) },
  };
  const deps = {
    currentVersion: async () => "0.5.11",
    check: async () => metadata,
    download: async () => { downloads++; return { path: "/cache/update.dmg" }; },
    install: async (version, automatic) => { installs.push({ version, automatic }); return true; },
    enabled: async () => true,
    state: (s) => states.push(s),
    checksumMissing: () => "checksum missing",
    ...overrides,
  };
  return { run: createUpdateRunner(deps), states, installs, metadata, downloads: () => downloads };
}

test("automatic updates download, install, and restart without a prompt", async () => {
  const f = fixture();
  await f.run(true);
  assert.deepEqual(f.states.map(s => s.status), ["checking", "downloading", "installing"]);
  assert.deepEqual(f.installs, [{ version: "0.5.12", automatic: true }]);
});

test("manual check only downloads; explicit install works with auto updates off", async () => {
  const f = fixture({ enabled: async () => false });
  await f.run();
  assert.equal(f.states.at(-1).status, "downloaded");
  assert.equal(f.installs.length, 0);
  await f.run(false, true);
  assert.deepEqual(f.installs, [{ version: "0.5.12", automatic: false }]);
});

test("off disables automatic checks entirely", async () => {
  const f = fixture({ enabled: async () => false });
  await f.run(true);
  assert.equal(f.states.length, 0);
  assert.equal(f.downloads(), 0);
});

test("disabling updates while downloading cancels installation", async () => {
  let enabled = true;
  const f = fixture({ enabled: async () => enabled, download: async () => { enabled = false; return { path: "cached" }; } });
  await f.run(true);
  assert.equal(f.installs.length, 0);
  assert.equal(f.states.at(-1).status, "downloaded");
});

test("native cancellation leaves manual installation available", async () => {
  const f = fixture({ install: async () => false });
  await f.run(true);
  assert.equal(f.states.at(-1).status, "downloaded");
});

test("failed version stops automatic retries but permits explicit manual retry", async () => {
  const f = fixture();
  f.metadata.installError = "previous version restored";
  await f.run(true);
  assert.equal(f.states.at(-1).status, "error");
  assert.equal(f.downloads(), 0);
  await f.run(false, true);
  assert.equal(f.installs.length, 1);
});

test("installer errors retain DMG for the manual fallback", async () => {
  const f = fixture({ install: async () => { throw new Error("permission denied"); } });
  await f.run(true);
  assert.equal(f.states.at(-1).status, "error");
  assert.equal(f.states.at(-1).download.path, "/cache/update.dmg");
});

test("missing checksums and unavailable updates never install", async () => {
  for (const available of [false, true]) {
    const f = fixture();
    f.metadata.available = available;
    f.metadata.asset.sha256 = null;
    await f.run(true);
    assert.equal(f.downloads(), 0);
    assert.equal(f.installs.length, 0);
    assert.equal(f.states.at(-1).status, available ? "error" : "up-to-date");
  }
});

test("concurrent checks share one operation", async () => {
  let release;
  const f = fixture({ currentVersion: () => new Promise(resolve => { release = resolve; }) });
  const first = f.run();
  const second = f.run(true);
  assert.equal(first, second);
  release("0.5.11");
  await first;
  assert.equal(f.downloads(), 1);
});
