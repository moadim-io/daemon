#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

const expectedVersion = '1.5.0';

export function installedVersion(stdout) {
  const match = stdout.trim().match(/\b(\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?)\b/);
  return match?.[1] ?? null;
}

export function needsInstall(stdout, version) {
  return installedVersion(stdout) !== version;
}

export function cargoBinaryPath() {
  return join(process.env.CARGO_HOME ?? join(homedir(), '.cargo'), 'bin', 'moadim');
}

function run(command, args) {
  return spawnSync(command, args, { encoding: 'utf8' });
}

function installedBinaryVersion() {
  const result = run(cargoBinaryPath(), ['--version']);
  if (result.error || result.status !== 0) {
    return null;
  }

  return installedVersion(result.stdout);
}

function installCargoBinary() {
  const cargo = run('cargo', ['install', '--locked', '--force', '--version', expectedVersion, 'moadim']);
  if (cargo.error) {
    process.stderr.write('cargo is required to install moadim. Install Rust from https://rustup.rs/ and rerun `npm install -g moadim`.\n');
    return false;
  }

  if (cargo.status !== 0) {
    if (cargo.stderr) {
      process.stderr.write(cargo.stderr);
    }
    return false;
  }

  return true;
}

function runInstalledBinary(args) {
  const result = spawnSync(cargoBinaryPath(), args, { stdio: 'inherit' });
  return result.status ?? 1;
}

export function main(args = process.argv.slice(2)) {
  if (process.platform === 'win32') {
    process.stderr.write('moadim needs a Unix-like host with tmux and crontab.\n');
    return 1;
  }

  if (needsInstall(installedBinaryVersion() ?? '', expectedVersion) && !installCargoBinary()) {
    return 1;
  }

  return runInstalledBinary(args);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = main();
}
