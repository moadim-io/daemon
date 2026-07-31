const { existsSync } = require('node:fs');
const { join } = require('node:path');
const { arch, platform, report } = require('node:process');

const PACKAGES = Object.freeze({
  darwin: {
    arm64: { packageName: '@moadim/daemon-darwin-arm64', binary: 'moadim' },
    x64: { packageName: '@moadim/daemon-darwin-x64', binary: 'moadim' },
  },
  linux: {
    x64: {
      gnu: { packageName: '@moadim/daemon-linux-x64-gnu', binary: 'moadim' },
    },
  },
});

function linuxLibc() {
  if (process.env.MOADIM_NPM_LIBC) {
    return process.env.MOADIM_NPM_LIBC;
  }
  try {
    return report?.getReport?.().header?.glibcVersionRuntime ? 'gnu' : 'musl';
  } catch {
    return 'gnu';
  }
}

function packageFor(currentPlatform = platform, currentArch = arch) {
  const byArch = PACKAGES[currentPlatform]?.[currentArch];
  if (!byArch) {
    return null;
  }
  return currentPlatform === 'linux' ? byArch[linuxLibc()] ?? null : byArch;
}

function resolveBinary() {
  const selected = packageFor();
  if (!selected) {
    return { ok: false, reason: 'unsupported-platform', platform, arch, libc: platform === 'linux' ? linuxLibc() : undefined };
  }

  const localBinary = join(__dirname, '..', '..', selected.packageName.split('/').pop(), 'bin', selected.binary);
  if (existsSync(localBinary)) {
    return { ok: true, path: localBinary, packageName: selected.packageName };
  }

  try {
    return { ok: true, path: require.resolve(`${selected.packageName}/bin/${selected.binary}`), packageName: selected.packageName };
  } catch (error) {
    return { ok: false, reason: 'missing-optional-dependency', packageName: selected.packageName, error };
  }
}

function missingBinaryMessage(result) {
  if (result.reason === 'unsupported-platform') {
    return `Moadim does not publish an npm binary for ${result.platform}/${result.arch}${result.libc ? `/${result.libc}` : ''}. Download a release archive from https://github.com/moadim-io/daemon/releases instead.`;
  }
  return `Could not find ${result.packageName}, the optional npm package that contains the Moadim binary for this platform. Reinstall without --omit=optional, or download a release archive from https://github.com/moadim-io/daemon/releases.`;
}

module.exports = { PACKAGES, linuxLibc, packageFor, resolveBinary, missingBinaryMessage };
