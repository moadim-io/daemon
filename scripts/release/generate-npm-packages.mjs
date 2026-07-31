#!/usr/bin/env node
import { mkdirSync, readFileSync, writeFileSync, copyFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const NPM_DIR = path.join(ROOT, 'npm');
const MANIFEST_PATH = path.join(NPM_DIR, 'packages.json');

function readJson(file) {
  return JSON.parse(readFileSync(file, 'utf8'));
}

function readCargoField(field) {
  const cargoToml = readFileSync(path.join(ROOT, 'Cargo.toml'), 'utf8');
  const match = cargoToml.match(new RegExp(`^${field} = "([^"]+)"`, 'm'));
  return match?.[1];
}

function writeJson(file, value) {
  writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function syncLicense(destinationDir) {
  copyFileSync(path.join(ROOT, 'LICENSE'), path.join(destinationDir, 'LICENSE'));
}

function platformPackageJson(platform, version) {
  const pkg = {
    name: platform.name,
    version,
    description: `Prebuilt Moadim binary for ${platform.os} ${platform.cpu}${platform.libc ? ` ${platform.libc}` : ''}`,
    homepage: readCargoField('homepage'),
    license: readCargoField('license'),
    repository: {
      type: 'git',
      url: `${readCargoField('repository')}.git`,
    },
    os: [platform.os],
    cpu: [platform.cpu],
    files: ['bin/moadim', 'README.md', 'LICENSE'],
    publishConfig: {
      access: 'public',
      provenance: true,
    },
  };
  if (platform.libc) pkg.libc = [platform.libc];
  return pkg;
}

function platformReadme(platform) {
  return `# ${platform.name}\n\nPrebuilt Moadim binary for ${platform.os} ${platform.cpu}${platform.libc ? ` ${platform.libc}` : ''}.\n`;
}

export function manifest() {
  return readJson(MANIFEST_PATH);
}

export function cargoVersion() {
  const version = readCargoField('version');
  if (!version) throw new Error('Cargo.toml is missing a package version');
  return version;
}

export function platformDir(platform) {
  return path.join(NPM_DIR, platform.packageDir);
}

export function generatePlatformPackages() {
  const version = cargoVersion();
  const { platforms } = manifest();
  for (const platform of platforms) {
    const dir = platformDir(platform);
    mkdirSync(path.join(dir, 'bin'), { recursive: true });
    writeJson(path.join(dir, 'package.json'), platformPackageJson(platform, version));
    writeFileSync(path.join(dir, 'README.md'), platformReadme(platform));
    syncLicense(dir);
    const placeholder = path.join(dir, 'bin', '.gitkeep');
    if (!existsSync(path.join(dir, 'bin', 'moadim'))) {
      writeFileSync(placeholder, '');
    }
  }
  console.log(`Generated ${platforms.length} npm platform package(s) for ${version}.`);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  generatePlatformPackages();
}
