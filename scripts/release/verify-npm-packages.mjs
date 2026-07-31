#!/usr/bin/env node
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { cargoVersion, generatePlatformPackages, manifest, platformDir } from './generate-npm-packages.mjs';

const require = createRequire(import.meta.url);
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const ROOT_PACKAGE = path.join(ROOT, 'npm', 'moadim', 'package.json');
const NPM_WORKFLOW = path.join(ROOT, '.github', 'workflows', 'npm-packages.yml');

function readJson(file) {
  return JSON.parse(readFileSync(file, 'utf8'));
}

function packageDirs() {
  const { root, platforms } = manifest();
  return [
    path.join(ROOT, 'npm', root.packageDir),
    ...platforms.map((platform) => platformDir(platform)),
  ];
}

function verifyVersions(version) {
  const workspacePackage = readJson(path.join(ROOT, 'package.json'));
  if (workspacePackage.version !== version) {
    throw new Error(`root package.json version ${workspacePackage.version} != Cargo.toml ${version}`);
  }

  const { platforms } = manifest();
  const rootPackage = readJson(ROOT_PACKAGE);
  if (rootPackage.version !== version) {
    throw new Error(`npm/moadim version ${rootPackage.version} != Cargo.toml ${version}`);
  }

  const expectedOptional = Object.fromEntries(platforms.map((platform) => [platform.name, version]));
  const actualOptional = rootPackage.optionalDependencies ?? {};
  if (JSON.stringify(actualOptional) !== JSON.stringify(expectedOptional)) {
    throw new Error(`npm/moadim optionalDependencies do not match npm/packages.json for ${version}`);
  }

  for (const platform of platforms) {
    const pkg = readJson(path.join(platformDir(platform), 'package.json'));
    if (pkg.name !== platform.name) {
      throw new Error(`${platform.packageDir} package name ${pkg.name} != manifest ${platform.name}`);
    }
    if (pkg.version !== version) {
      throw new Error(`${platform.packageDir} version ${pkg.version} != Cargo.toml ${version}`);
    }
    if (pkg.os?.[0] !== platform.os || pkg.cpu?.[0] !== platform.cpu) {
      throw new Error(`${platform.packageDir} os/cpu do not match npm/packages.json`);
    }
    if ((pkg.libc?.[0] ?? undefined) !== platform.libc) {
      throw new Error(`${platform.packageDir} libc does not match npm/packages.json`);
    }
  }
}

function wrapperLibc(platform) {
  if (platform.os !== 'linux') return undefined;
  if (platform.libc === 'glibc') return 'gnu';
  return platform.libc;
}

function verifyWrapperManifestSync() {
  const { PACKAGES, packageFor } = require(path.join(ROOT, 'npm', 'moadim', 'lib', 'platform.cjs'));
  const { platforms } = manifest();
  const expectedNames = new Set(platforms.map((platform) => platform.name));
  const actualNames = new Set();

  for (const byArch of Object.values(PACKAGES)) {
    for (const entry of Object.values(byArch)) {
      const candidates = entry.packageName ? [entry] : Object.values(entry);
      for (const candidate of candidates) {
        actualNames.add(candidate.packageName);
      }
    }
  }

  for (const expectedName of expectedNames) {
    if (!actualNames.has(expectedName)) {
      throw new Error(`npm wrapper does not declare ${expectedName} from npm/packages.json`);
    }
  }
  for (const actualName of actualNames) {
    if (!expectedNames.has(actualName)) {
      throw new Error(`npm wrapper declares ${actualName}, but npm/packages.json does not`);
    }
  }

  const previousLibc = process.env.MOADIM_NPM_LIBC;
  try {
    for (const platform of platforms) {
      if (platform.os === 'linux') {
        process.env.MOADIM_NPM_LIBC = wrapperLibc(platform);
      } else {
        delete process.env.MOADIM_NPM_LIBC;
      }
      const selected = packageFor(platform.os, platform.cpu);
      if (selected?.packageName !== platform.name) {
        throw new Error(`npm wrapper selects ${selected?.packageName ?? 'nothing'} for ${platform.os}/${platform.cpu}, expected ${platform.name}`);
      }
    }
  } finally {
    if (previousLibc === undefined) {
      delete process.env.MOADIM_NPM_LIBC;
    } else {
      process.env.MOADIM_NPM_LIBC = previousLibc;
    }
  }
}

function verifyWorkflowManifestSync() {
  const workflow = readFileSync(NPM_WORKFLOW, 'utf8');
  const { platforms } = manifest();
  for (const platform of platforms) {
    if (!workflow.includes(`target: ${platform.target}`)) {
      throw new Error(`npm workflow matrix is missing target ${platform.target} from npm/packages.json`);
    }
    if (!workflow.includes(`package_dir: ${platform.packageDir}`)) {
      throw new Error(`npm workflow build matrix is missing ${platform.packageDir} from npm/packages.json`);
    }
    if (!workflow.includes(`- ${platform.packageDir}`)) {
      throw new Error(`npm workflow publish matrix is missing ${platform.packageDir} from npm/packages.json`);
    }
  }
}

function verifyPackContents() {
  const requirePlatformBinaries = process.env.MOADIM_REQUIRE_NPM_BINARIES === '1';
  const { root, platforms } = manifest();
  const requiredPackageDir = process.env.MOADIM_REQUIRED_NPM_PACKAGE;
  if (requiredPackageDir && !platforms.some((platform) => platform.packageDir === requiredPackageDir)) {
    throw new Error(`${requiredPackageDir} is not declared in npm/packages.json`);
  }
  const platformDirs = new Set(platforms.map((platform) => platformDir(platform)));
  const requiredPlatformDirs = new Set(
    requirePlatformBinaries
      ? platforms
          .filter((platform) => !requiredPackageDir || platform.packageDir === requiredPackageDir)
          .map((platform) => platformDir(platform))
      : [],
  );

  for (const dir of packageDirs()) {
    const output = execFileSync('npm', ['pack', '--json', '--dry-run'], { cwd: dir, encoding: 'utf8' });
    const [{ files }] = JSON.parse(output);
    const names = files.map((file) => file.path).sort();
    if (!names.includes('package.json')) throw new Error(`${dir} pack is missing package.json`);
    if (requirePlatformBinaries && requiredPlatformDirs.has(dir) && !names.includes('bin/moadim')) {
      throw new Error(`${dir} pack is missing staged bin/moadim`);
    }
    if (!platformDirs.has(dir) && path.basename(dir) !== root.packageDir) {
      throw new Error(`${dir} is not declared in npm/packages.json`);
    }
    for (const forbidden of ['Cargo.toml', '.env', 'node_modules/', 'target/']) {
      if (names.some((name) => name === forbidden || name.startsWith(forbidden))) {
        throw new Error(`${dir} pack includes forbidden path ${forbidden}`);
      }
    }
  }
}

function main() {
  generatePlatformPackages();
  const version = cargoVersion();
  verifyVersions(version);
  verifyWrapperManifestSync();
  verifyWorkflowManifestSync();
  verifyPackContents();
  const binaryMode = process.env.MOADIM_REQUIRE_NPM_BINARIES === '1' ? ' with staged platform binaries' : '';
  console.log(`npm package versions and pack contents${binaryMode} match Cargo.toml ${version}.`);
}

main();
