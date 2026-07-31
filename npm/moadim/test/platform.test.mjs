import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { test } from 'node:test';

const require = createRequire(import.meta.url);
const { missingBinaryMessage, packageFor } = require('../lib/platform.cjs');

test('selects macOS packages by architecture', () => {
  assert.equal(packageFor('darwin', 'arm64').packageName, '@moadim/daemon-darwin-arm64');
  assert.equal(packageFor('darwin', 'x64').packageName, '@moadim/daemon-darwin-x64');
});

test('selects Linux glibc package and rejects unsupported musl', () => {
  process.env.MOADIM_NPM_LIBC = 'gnu';
  assert.equal(packageFor('linux', 'x64').packageName, '@moadim/daemon-linux-x64-gnu');
  process.env.MOADIM_NPM_LIBC = 'musl';
  assert.equal(packageFor('linux', 'x64'), null);
  delete process.env.MOADIM_NPM_LIBC;
});

test('returns null for unsupported platforms', () => {
  assert.equal(packageFor('win32', 'x64'), null);
});

test('missing optional dependency error explains omit optional', () => {
  const message = missingBinaryMessage({ reason: 'missing-optional-dependency', packageName: '@moadim/daemon-linux-x64-gnu' });
  assert.match(message, /--omit=optional/);
  assert.match(message, /GitHub Releases|release archive/);
});
