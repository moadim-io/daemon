#!/usr/bin/env node
const { spawnSync } = require('node:child_process');
const { resolveBinary, missingBinaryMessage } = require('../lib/platform.cjs');

const resolved = resolveBinary();
if (!resolved.ok) {
  console.error(missingBinaryMessage(resolved));
  process.exit(1);
}

const child = spawnSync(resolved.path, process.argv.slice(2), {
  stdio: 'inherit',
  windowsHide: false,
});

if (child.error) {
  console.error(`Failed to run ${resolved.path}: ${child.error.message}`);
  process.exit(1);
}

if (child.signal) {
  process.kill(process.pid, child.signal);
}

process.exit(child.status ?? 1);
