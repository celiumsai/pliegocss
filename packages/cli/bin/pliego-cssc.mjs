#!/usr/bin/env node

import { runNative } from "./launcher.mjs";

try {
  process.exitCode = runNative("pliego-cssc", process.argv.slice(2));
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
