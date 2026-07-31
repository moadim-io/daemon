#!/usr/bin/env node
import { copyFileSync, chmodSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { generatePlatformPackages, manifest } from './generate-npm-packages.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

function usage() {
  console.error('usage: node scripts/release/stage-npm-binaries.mjs <target-triple> <path-to-moadim-binary>');
  process.exit(2);
}

const [target, binaryPath] = process.argv.slice(2);
if (!target || !binaryPath) usage();

generatePlatformPackages();

const platform = manifest().platforms.find((candidate) => candidate.target === target);
if (!platform) {
  throw new Error(`No npm platform package is configured for target ${target}`);
}

const destinationDir = path.join(ROOT, 'npm', platform.packageDir, 'bin');
const destination = path.join(destinationDir, process.platform === 'win32' ? 'moadim.exe' : 'moadim');
mkdirSync(destinationDir, { recursive: true });
copyFileSync(path.resolve(binaryPath), destination);
chmodSync(destination, 0o755);
console.log(`Staged ${target} binary into ${path.relative(ROOT, destination)}`);
