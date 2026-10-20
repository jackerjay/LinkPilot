import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { isProfileAccessDenied, profileScanError } from '../src/lib/profile-access.ts';

test('serialized permission denials remain actionable after the IPC boundary', () => {
  const error = profileScanError({ permission_denied: true, message: 'io error: access denied' });
  assert.ok(isProfileAccessDenied(error));
  assert.equal(error.message, 'io error: access denied');
});

test('invalid profile data does not offer permission recovery', () => {
  const error = profileScanError({ permission_denied: false, message: 'invalid profile JSON' });
  assert.equal(isProfileAccessDenied(error), false);
  assert.equal(error.message, 'invalid profile JSON');
});

test('messages alone and malformed payloads never imply a permission denial', () => {
  for (const value of ['Operation not permitted (os error 1)', { permission_denied: 'true', message: 'denied' }, null]) {
    assert.equal(isProfileAccessDenied(profileScanError(value)), false);
  }
});

test('transport errors preserve their original details', () => {
  const error = new Error('connection lost');
  assert.equal(profileScanError(error), error);
});

test('permission recovery is translated in every supported language', () => {
  const read = locale => JSON.parse(readFileSync(new URL(`../src/i18n/locales/${locale}/browsers.json`, import.meta.url))).profileAccess;
  const expected = Object.keys(read('en')).sort();
  for (const locale of ['en', 'zh-CN', 'zh-TW', 'ja-JP']) {
    const copy = read(locale);
    assert.deepEqual(Object.keys(copy).sort(), expected);
    for (const key of ['title', 'description', 'instructions']) assert.ok(copy[key].includes('{{name}}'));
  }
});
