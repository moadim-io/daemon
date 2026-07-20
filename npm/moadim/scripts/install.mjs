#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";

const expectedVersion = "1.5.0";

export function installedVersion(stdout) {
	const match = stdout
		.trim()
		.match(/\b(\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?)\b/);
	return match?.[1] ?? null;
}

export function needsInstall(stdout, version) {
	return installedVersion(stdout) !== version;
}

function run(command, args) {
	return spawnSync(command, args, { encoding: "utf8" });
}

function checkInstalled() {
	const result = run("moadim", ["--version"]);
	if (result.error || result.status !== 0) {
		return null;
	}

	return installedVersion(result.stdout);
}

export function main() {
	if (process.platform === "win32") {
		process.stderr.write(
			"moadim needs a Unix-like host with tmux and crontab.\n",
		);
		return 1;
	}

	const currentVersion = checkInstalled();
	if (currentVersion === expectedVersion) {
		return 0;
	}

	const cargo = run("cargo", [
		"install",
		"--locked",
		"--force",
		"--version",
		expectedVersion,
		"moadim",
	]);
	if (cargo.error) {
		process.stderr.write(
			"cargo is required to install moadim. Install Rust from https://rustup.rs/ and rerun `moadim-install`.\n",
		);
		return 1;
	}

	if (cargo.status !== 0) {
		if (cargo.stderr) {
			process.stderr.write(cargo.stderr);
		}
		return cargo.status ?? 1;
	}

	return 0;
}

if (
	process.argv[1] &&
	import.meta.url === pathToFileURL(process.argv[1]).href
) {
	process.exitCode = main();
}
