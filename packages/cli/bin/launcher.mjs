import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const PACKAGE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export const SUPPORTED_TARGETS = Object.freeze({
  "darwin:arm64": Object.freeze({
    id: "aarch64-apple-darwin",
    extension: "",
  }),
  "linux:x64": Object.freeze({
    id: "x86_64-unknown-linux-gnu",
    extension: "",
  }),
  "win32:x64": Object.freeze({
    id: "x86_64-pc-windows-msvc",
    extension: ".exe",
  }),
});

const BINARY_NAMES = new Set(["pliego-cssc", "pliego-css-lsp"]);

export function selectTarget(platform, arch) {
  const target = SUPPORTED_TARGETS[`${platform}:${arch}`];
  if (!target) {
    const supported = Object.keys(SUPPORTED_TARGETS).sort().join(", ");
    throw new Error(
      `PliegoCSS has no repository binary for ${platform}:${arch}; supported hosts: ${supported}`,
    );
  }
  return target;
}

export function resolveBinary(packageRoot, binaryName, platform, arch) {
  if (!BINARY_NAMES.has(binaryName)) {
    throw new Error(`unknown PliegoCSS repository binary: ${binaryName}`);
  }
  const target = selectTarget(platform, arch);
  return resolve(
    packageRoot,
    "vendor",
    target.id,
    `${binaryName}${target.extension}`,
  );
}

export function runNative(binaryName, args) {
  const binary = resolveBinary(
    PACKAGE_ROOT,
    binaryName,
    process.platform,
    process.arch,
  );
  if (!existsSync(binary)) {
    throw new Error(
      `PliegoCSS package is incomplete: ${binaryName} is missing for ${process.platform}:${process.arch}`,
    );
  }
  const result = spawnSync(binary, args, {
    shell: false,
    stdio: "inherit",
    windowsHide: false,
  });
  if (result.error) throw result.error;
  if (result.signal) {
    throw new Error(`${binaryName} terminated by signal ${result.signal}`);
  }
  return result.status ?? 1;
}
