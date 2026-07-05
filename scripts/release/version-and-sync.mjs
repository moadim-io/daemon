#!/usr/bin/env node
// Wraps `changeset version` so a version bump also drives the parts of the
// release this repo actually cares about: `Cargo.toml`/`Cargo.lock` (the real
// version source `auto-release.yml`/`publish.yml`/`release.yml` read) and
// `CHANGELOG.md` in this repo's bracketed/dated Keep-a-Changelog style, which
// `@changesets/cli`'s own changelog writer can't produce (see
// `.changeset/config.json`'s `"changelog": false`).
//
// Must run BEFORE `changeset version`, which deletes the consumed
// `.changeset/*.md` files — their bodies are the only record of what each
// changeset said once that happens.

import { execSync } from "node:child_process";
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const CHANGESET_DIR = path.join(ROOT, ".changeset");
const CHANGELOG_PATH = path.join(ROOT, "CHANGELOG.md");
const PACKAGE_JSON_PATH = path.join(ROOT, "package.json");
const CARGO_TOML_PATH = path.join(ROOT, "Cargo.toml");
const MAN_PAGE_PATH = path.join(ROOT, "docs", "moadim.1");
const REPO_URL = "https://github.com/moadim-io/daemon";

function readPendingChangesets() {
  const files = readdirSync(CHANGESET_DIR)
    .filter((f) => f.endsWith(".md") && f !== "README.md")
    .sort();
  return files.map((file) => {
    const raw = readFileSync(path.join(CHANGESET_DIR, file), "utf8");
    const match = raw.match(/^---\n[\s\S]*?\n---\n([\s\S]*)$/);
    if (!match) {
      throw new Error(`Malformed changeset file: ${file}`);
    }
    return { file, body: match[1].trim() };
  });
}

function bumpCargoToml(version) {
  const cargoToml = readFileSync(CARGO_TOML_PATH, "utf8");
  let replaced = false;
  const next = cargoToml.replace(/^version = "[^"]*"/m, () => {
    replaced = true;
    return `version = "${version}"`;
  });
  if (!replaced) {
    throw new Error(`Could not find a "version" line in ${CARGO_TOML_PATH}`);
  }
  writeFileSync(CARGO_TOML_PATH, next);
}

// docs/moadim.1 hand-mirrors the CLI and hardcodes its own version in the
// .TH header (see the `man_page_version_matches_cargo_pkg_version` test in
// src/cli_tests.rs, which fails the build if this drifts from Cargo.toml).
function bumpManPage(version) {
  const manPage = readFileSync(MAN_PAGE_PATH, "utf8");
  let replaced = false;
  const next = manPage.replace(/^(\.TH MOADIM 1 "[^"]*" ")moadim [^"]*(")/m, (_, pre, post) => {
    replaced = true;
    return `${pre}moadim ${version}${post}`;
  });
  if (!replaced) {
    throw new Error(`Could not find a .TH header line in ${MAN_PAGE_PATH}`);
  }
  writeFileSync(MAN_PAGE_PATH, next);
}

function todayIso() {
  return new Date().toISOString().slice(0, 10);
}

function updateChangelog(version, changesetBody) {
  const changelog = readFileSync(CHANGELOG_PATH, "utf8");

  const unreleasedMatch = changelog.match(/^## \[Unreleased\]\n/m);
  if (!unreleasedMatch) {
    throw new Error('CHANGELOG.md is missing a "## [Unreleased]" heading');
  }
  const unreleasedStart = unreleasedMatch.index;
  const bodyStart = unreleasedStart + unreleasedMatch[0].length;

  const nextHeadingMatch = changelog.slice(bodyStart).match(/\n## \[/);
  if (!nextHeadingMatch) {
    throw new Error("CHANGELOG.md has no dated release heading after Unreleased");
  }
  const firstSectionStart = bodyStart + nextHeadingMatch.index + 1;

  const footerMarker = "\n[Unreleased]:";
  const footerStart = changelog.indexOf(footerMarker);
  if (footerStart === -1) {
    throw new Error("CHANGELOG.md is missing its compare-link footer");
  }

  const preamble = changelog.slice(0, unreleasedStart);
  const restOfSections = changelog.slice(firstSectionStart, footerStart);

  // Anything still hand-written directly under Unreleased (pre-migration
  // entries, or a one-off bypass of the changeset gate) ships too — the
  // changeset files being consumed here are additive, not a replacement.
  const existingUnreleasedBody = changelog.slice(bodyStart, firstSectionStart).trim();
  const newSectionBody = [existingUnreleasedBody, changesetBody].filter(Boolean).join("\n\n");

  const dated = `## [${version}] - ${todayIso()}\n\n${newSectionBody}\n`;

  const bodyWithoutFooter = `${preamble}## [Unreleased]\n\n${dated}\n${restOfSections}`;

  // Recompute the whole compare-link footer from every dated heading actually
  // present in the file, oldest-last — repairs drift instead of trusting the
  // previous footer, which had gone stale (stuck at v0.15.0 with headings past
  // v0.18.0).
  const headingRe = /^## \[(\d+\.\d+\.\d+)\]/gm;
  const versions = [...bodyWithoutFooter.matchAll(headingRe)]
    .map((m) => m[1])
    .filter((v) => v !== "Unreleased");

  const footerLines = [`[Unreleased]: ${REPO_URL}/compare/v${versions[0]}...HEAD`];
  for (let i = 0; i < versions.length; i++) {
    const v = versions[i];
    const prev = versions[i + 1];
    footerLines.push(
      prev
        ? `[${v}]: ${REPO_URL}/compare/v${prev}...v${v}`
        : `[${v}]: ${REPO_URL}/releases/tag/v${v}`,
    );
  }

  writeFileSync(CHANGELOG_PATH, `${bodyWithoutFooter}\n${footerLines.join("\n")}\n`);
}

function main() {
  const pending = readPendingChangesets();
  if (pending.length === 0) {
    console.log("No pending changesets — nothing to release.");
    return;
  }

  const combinedBody = pending.map((c) => c.body).join("\n\n");

  execSync("pnpm exec changeset version", { cwd: ROOT, stdio: "inherit" });

  const pkg = JSON.parse(readFileSync(PACKAGE_JSON_PATH, "utf8"));
  const version = pkg.version;

  updateChangelog(version, combinedBody);
  bumpCargoToml(version);
  bumpManPage(version);

  execSync("cargo check -q", { cwd: ROOT, stdio: "inherit" });

  // The committed OpenAPI spec embeds the crate version. Run the self-healing
  // test once (it regenerates apis/openapi.json and exits non-zero on drift),
  // then again to confirm the regenerated file is stable.
  try {
    execSync(
      "cargo test --quiet -- openapi::openapi_tests::committed_spec_is_current",
      { cwd: ROOT, stdio: "inherit" },
    );
  } catch {
    execSync(
      "cargo test --quiet -- openapi::openapi_tests::committed_spec_is_current",
      { cwd: ROOT, stdio: "inherit" },
    );
  }

  console.log(`Synced Cargo.toml, Cargo.lock, CHANGELOG.md, docs/moadim.1, and apis/openapi.json to ${version}.`);
}

main();
