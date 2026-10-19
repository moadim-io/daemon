import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const config = JSON.parse(
  readFileSync(new URL("../../.changeset/config.json", import.meta.url), "utf8"),
);

test("changesets versions the private daemon package", () => {
  assert.deepEqual(config.privatePackages, {
    version: true,
    tag: false,
  });
});
