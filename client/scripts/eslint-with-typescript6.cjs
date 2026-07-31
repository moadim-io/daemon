#!/usr/bin/env node
/* eslint-disable @typescript-eslint/no-require-imports, no-undef */
const Module = require('node:module');

const ts6Entry = require.resolve('@typescript/typescript6');
const originalResolveFilename = Module._resolveFilename;

Module._resolveFilename = function resolveWithTypescript6(request, parent, isMain, options) {
  if (request === 'typescript') {
    return ts6Entry;
  }

  return originalResolveFilename.call(this, request, parent, isMain, options);
};

const path = require('node:path');
const eslintPkg = require.resolve('eslint/package.json');
const eslintBin = path.join(path.dirname(eslintPkg), 'bin', 'eslint.js');
process.argv = [process.argv[0], eslintBin, ...process.argv.slice(2)];
require(eslintBin);
