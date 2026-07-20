import assert from "node:assert/strict";
import { installedVersion, needsInstall } from "./install.mjs";

assert.equal(installedVersion("moadim 1.5.0"), "1.5.0");
assert.equal(installedVersion("moadim 1.5.0 (abc123)"), "1.5.0");
assert.equal(installedVersion(""), null);
assert.equal(needsInstall("moadim 1.5.0", "1.5.0"), false);
assert.equal(needsInstall("", "1.5.0"), true);
