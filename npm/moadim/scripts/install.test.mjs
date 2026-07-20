import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, rmSync, readFileSync, writeFileSync, chmodSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { cargoBinaryPath, installedVersion, main, needsInstall } from './install.mjs';

assert.equal(installedVersion('moadim 1.5.0'), '1.5.0');
assert.equal(installedVersion('moadim 1.5.0 (abc123)'), '1.5.0');
assert.equal(installedVersion(''), null);
assert.equal(needsInstall('moadim 1.5.0', '1.5.0'), false);
assert.equal(needsInstall('', '1.5.0'), true);

const home = mkdtempSync(join(tmpdir(), 'moadim-npm-'));
const binDir = join(home, '.cargo', 'bin');
mkdirSync(binDir, { recursive: true });
const cargoBinary = join(binDir, 'moadim');
writeFileSync(
  cargoBinary,
  '#!/bin/sh\nif [ "$1" = "--version" ]; then\n  echo "moadim 1.5.0"\nelse\n  echo "$@" > "$MOADIM_ARGS_FILE"\nfi\n',
);
chmodSync(cargoBinary, 0o755);

const originalHome = process.env.HOME;
const originalCargoHome = process.env.CARGO_HOME;
const originalPath = process.env.PATH;
const originalArgs = process.env.MOADIM_ARGS_FILE;
process.env.HOME = home;
delete process.env.CARGO_HOME;
process.env.PATH = '';
process.env.MOADIM_ARGS_FILE = join(home, 'args.txt');
assert.equal(cargoBinaryPath(), cargoBinary);
assert.equal(main(['--help']), 0);
assert.equal(readFileSync(join(home, 'args.txt'), 'utf8').trim(), '--help');
process.env.HOME = originalHome;
process.env.CARGO_HOME = originalCargoHome;
process.env.PATH = originalPath;
if (originalArgs === undefined) {
  delete process.env.MOADIM_ARGS_FILE;
} else {
  process.env.MOADIM_ARGS_FILE = originalArgs;
}
rmSync(home, { recursive: true, force: true });
