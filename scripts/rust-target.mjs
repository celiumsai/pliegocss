import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { isAbsolute, join, resolve } from "node:path";

function absoluteTargetBase(root, env) {
  const configured = env.PLIEGOCSS_TARGET_BASE ?? env.CARGO_TARGET_DIR;
  if (!configured) return join(root, "target");
  return isAbsolute(configured) ? configured : resolve(root, configured);
}

function safeSegment(value) {
  return value.toLowerCase().replaceAll(/[^a-z0-9._-]+/gu, "-").replaceAll(/^-+|-+$/gu, "");
}

export function targetDirectoryForRustcIdentity(root, env, identity) {
  const digest = createHash("sha256").update(identity).digest("hex");
  const release = identity.match(/^release:\s*(.+)$/mu)?.[1] ?? "unknown";
  const host = identity.match(/^host:\s*(.+)$/mu)?.[1] ?? `${process.platform}-${process.arch}`;
  const segment = `${safeSegment(host)}-rustc-${safeSegment(release)}-${digest.slice(0, 16)}`;
  return join(absoluteTargetBase(root, env), "toolchains", segment);
}

function rustcIdentity(env, toolchain) {
  const command = toolchain ? "rustup" : (env.RUSTC ?? "rustc");
  const args = toolchain ? ["run", toolchain, "rustc", "-Vv"] : ["-Vv"];
  const result = spawnSync(command, args, {
    env,
    encoding: "utf8",
    windowsHide: true,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`cannot identify Rust toolchain (${command} ${args.join(" ")}): ${result.stderr}`);
  }
  return result.stdout.replaceAll("\r\n", "\n").trim();
}

export function isolatedCargoEnvironment(root, options = {}) {
  const env = options.env ?? process.env;
  if (env.PLIEGOCSS_ISOLATED_TARGET === "1" && !options.toolchain) return { ...env };
  const identity = rustcIdentity(env, options.toolchain);
  const target = targetDirectoryForRustcIdentity(root, env, identity);
  return {
    ...env,
    CARGO_TARGET_DIR: target,
    PLIEGOCSS_TARGET_BASE: absoluteTargetBase(root, env),
    PLIEGOCSS_ISOLATED_TARGET: "1",
    PLIEGOCSS_RUSTC_IDENTITY_SHA256: createHash("sha256").update(identity).digest("hex"),
  };
}

export function cargoTargetRoot(root, env = process.env) {
  const configured = env.CARGO_TARGET_DIR;
  if (!configured) return join(root, "target");
  return isAbsolute(configured) ? configured : resolve(root, configured);
}
