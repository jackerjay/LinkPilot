import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const script = fileURLToPath(new URL('./notarize-mac.sh', import.meta.url));
const fakeXcrun = `#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');
const args = process.argv.slice(2);
fs.appendFileSync(path.join(process.env.TEST_ROOT, 'calls'), JSON.stringify(args) + '\\n');
if (args[1] === 'submit') {
  console.log(JSON.stringify({ id: 'test-submission', status: process.env.TEST_STATUS }));
  process.exit(Number(process.env.TEST_SUBMIT_EXIT));
}
if (args[1] === 'log') fs.writeFileSync(args.at(-1), JSON.stringify({ issues: [] }));
if (args[0] === 'stapler' && args[1] === process.env.TEST_STAPLER_FAIL) process.exit(1);
`;

function run(t, env = {}) {
  const root = mkdtempSync(join(tmpdir(), 'linkpilot notary test-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const bin = join(root, 'bin');
  mkdirSync(bin);
  writeFileSync(join(bin, 'xcrun'), fakeXcrun, { mode: 0o755 });
  const result = spawnSync('/bin/bash', [script, 'LinkPilot.zip', 'LinkPilot.app'], {
    encoding: 'utf8',
    env: {
      ...process.env,
      PATH: `${bin}:${process.env.PATH}`,
      APPLE_KEYCHAIN: join(root, 'signing.keychain-db'),
      TEST_ROOT: root,
      TEST_STATUS: 'Accepted',
      TEST_SUBMIT_EXIT: '0',
      TEST_STAPLER_FAIL: '',
      ...env,
    },
  });
  const calls = readFileSync(join(root, 'calls'), 'utf8').trim().split('\n').map(JSON.parse);
  return { ...result, calls };
}

test('accepted submissions download diagnostics, staple, and validate the ticket', t => {
  const result = run(t);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(result.calls.map(args => args.slice(0, 2)), [
    ['notarytool', 'submit'], ['notarytool', 'log'], ['stapler', 'staple'], ['stapler', 'validate'],
  ]);
  assert.ok(result.calls[0].includes('--wait'));
  assert.deepEqual(result.calls[2].slice(2), ['LinkPilot.app']);
});

test('an uploaded but rejected submission fails after exposing Apple diagnostics', t => {
  const result = run(t, { TEST_STATUS: 'Invalid' });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /publication stopped/);
  assert.deepEqual(result.calls.map(args => args[1]), ['submit', 'log']);
  assert.match(result.stdout, /"issues"/);
});

test('a failed submit command cannot be treated as an accepted release', t => {
  const result = run(t, { TEST_SUBMIT_EXIT: '1' });
  assert.notEqual(result.status, 0);
  assert.deepEqual(result.calls.map(args => args[1]), ['submit', 'log']);
});

test('stapling or ticket validation failures fail the release', t => {
  for (const step of ['staple', 'validate']) {
    const result = run(t, { TEST_STAPLER_FAIL: step });
    assert.notEqual(result.status, 0);
    assert.equal(result.calls.at(-1)[1], step);
  }
});
