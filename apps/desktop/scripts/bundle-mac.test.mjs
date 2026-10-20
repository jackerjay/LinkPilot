import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const script = fileURLToPath(new URL('./bundle-mac.sh', import.meta.url));
const identity = 'A'.repeat(40);
const identities = `  1) ${identity} "Apple Development: Test"\n`;

// Compile/package tools are isolated; plist patching uses the real macOS tool.
const fakeTool = `#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');
const command = path.basename(process.argv[1]);
const args = process.argv.slice(2);
const root = process.env.TEST_ROOT;
fs.appendFileSync(path.join(root, 'calls'), JSON.stringify({ command, args }) + '\\n');
const release = path.join(root, 'target/release');
const app = path.join(release, 'bundle/macos/LinkPilot.app');
if (command === 'security') console.log(process.env.TEST_IDENTITIES);
if (command === 'cargo' && args[0] === 'metadata') {
  console.log(JSON.stringify({ target_directory: path.join(root, 'target') }));
} else if (command === 'cargo') {
  fs.mkdirSync(release, { recursive: true });
  for (const name of ['lpt', 'linkpilot-daemon']) fs.writeFileSync(path.join(release, name), name);
} else if (command === 'npm') {
  fs.mkdirSync(path.join(app, 'Contents/MacOS'), { recursive: true });
  fs.writeFileSync(path.join(app, 'Contents/Info.plist'),
    '<?xml version="1.0"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>CFBundleURLTypes</key><array><dict><key>CFBundleTypeRole</key><string>Editor</string></dict></array></dict></plist>');
} else if (command === 'codesign') {
  if (args.includes('--verify') && process.env.TEST_VERIFY_FAIL === '1') process.exit(1);
}
`;

function run(t, env = {}) {
  const root = mkdtempSync(join(tmpdir(), 'linkpilot bundle test-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const bin = join(root, 'bin');
  mkdirSync(bin);
  const tool = join(bin, 'tool.cjs');
  writeFileSync(tool, fakeTool, { mode: 0o755 });
  for (const name of ['cargo', 'npm', 'security', 'codesign']) symlinkSync(tool, join(bin, name));
  const result = spawnSync('/bin/bash', [script], {
    encoding: 'utf8',
    env: {
      ...process.env,
      PATH: `${bin}:${process.env.PATH}`,
      TEST_ROOT: root,
      TEST_IDENTITIES: identities,
      TEST_VERIFY_FAIL: '',
      APPLE_SIGNING_IDENTITY: '',
      ...env,
    },
  });
  const calls = readFileSync(join(root, 'calls'), 'utf8').trim().split('\n').map(JSON.parse);
  return { ...result, root, calls };
}

const macOnly = { skip: process.platform !== 'darwin' };

test('embeds helpers, patches browser registration, and signs inside out', macOnly, t => {
  const result = run(t);
  assert.equal(result.status, 0, result.stderr);
  const app = join(result.root, 'target/release/bundle/macos/LinkPilot.app');
  for (const name of ['lpt', 'linkpilot-daemon']) {
    assert.equal(readFileSync(join(app, 'Contents/MacOS', name), 'utf8'), name);
  }
  const plist = spawnSync('plutil', ['-convert', 'json', '-o', '-', join(app, 'Contents/Info.plist')], { encoding: 'utf8' });
  const info = JSON.parse(plist.stdout);
  assert.equal(info.CFBundleURLTypes[0].CFBundleTypeRole, 'Viewer');
  assert.equal(info.LSUIElement, true);
  const signing = result.calls.filter(call => call.command === 'codesign');
  assert.deepEqual(signing.map(call => call.args.at(-1)), [
    join(app, 'Contents/MacOS/lpt'), join(app, 'Contents/MacOS/linkpilot-daemon'), app, app,
  ]);
  assert.ok(signing.slice(0, 3).every(call => call.args[call.args.indexOf('--sign') + 1] === identity));
  assert.ok(signing[3].args.includes('--strict'));
  assert.ok(result.calls.find(call => call.command === 'npm').args.includes('--no-sign'));
});

test('multiple identities stop before compiling instead of choosing one', macOnly, t => {
  const result = run(t, { TEST_IDENTITIES: identities + `  2) ${'B'.repeat(40)} "Developer ID Application: Test"` });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /set APPLE_SIGNING_IDENTITY/);
  assert.deepEqual(result.calls.map(call => call.command), ['security']);
});

test('no certificate keeps ad-hoc builds available with an explicit notice', macOnly, t => {
  const result = run(t, { TEST_IDENTITIES: '0 valid identities found' });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stderr, /using ad-hoc signing/);
  const signing = result.calls.filter(call => call.args.includes('--sign'));
  assert.equal(signing.length, 3);
  assert.ok(signing.every(call => call.args[call.args.indexOf('--sign') + 1] === '-'));
});

test('an explicit signing identity overrides automatic selection', macOnly, t => {
  const result = run(t, { APPLE_SIGNING_IDENTITY: 'Apple Development: Selected' });
  assert.equal(result.status, 0, result.stderr);
  assert.ok(!result.calls.some(call => call.command === 'security'));
  const signing = result.calls.filter(call => call.args.includes('--sign'));
  assert.ok(signing.every(call => call.args[call.args.indexOf('--sign') + 1] === 'Apple Development: Selected'));
});

test('failed seal verification fails the build and never reports success', macOnly, t => {
  const result = run(t, { TEST_VERIFY_FAIL: '1' });
  assert.notEqual(result.status, 0);
  assert.doesNotMatch(result.stdout, /bundle:mac: verified/);
});
