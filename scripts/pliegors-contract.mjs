import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";

export const PLIEGORS_CONTRACT_SURFACES = Object.freeze([
  "Cargo.toml",
  "Cargo.lock",
  "crates/pliego-reactive/**",
  "crates/pliego-dom/**",
  "crates/pliego-macros/**",
  "crates/pliego-resume/**",
  "crates/pliego-ssg/**",
  "crates/pliego-starters/**",
  "crates/pliego-cli/**",
]);

function git(root, args, encoding = null) {
  const result = spawnSync("git", args, {
    cwd: root,
    encoding,
    maxBuffer: 64 * 1024 * 1024,
    windowsHide: true,
  });
  if (result.error) throw new Error(`cannot run git: ${result.error.message}`);
  if (result.status !== 0) {
    const stdout = result.stdout?.toString("utf8") ?? "";
    const stderr = result.stderr?.toString("utf8") ?? "";
    throw new Error(`git ${args.join(" ")} failed\n${stdout}${stderr}`);
  }
  return result.stdout;
}

function surfaceRoots() {
  return PLIEGORS_CONTRACT_SURFACES.map((surface) =>
    surface.endsWith("/**") ? surface.slice(0, -3) : surface,
  );
}

function committedSurfaceFiles(root, revision) {
  const output = git(
    root,
    ["ls-tree", "-r", "--name-only", "-z", revision, "--", ...surfaceRoots()],
    null,
  );
  const files = output
    .toString("utf8")
    .split("\0")
    .filter(Boolean)
    .sort((left, right) => left.localeCompare(right, "en"));
  for (const surface of PLIEGORS_CONTRACT_SURFACES) {
    const rootPath = surface.endsWith("/**") ? surface.slice(0, -3) : surface;
    const present = surface.endsWith("/**")
      ? files.some((file) => file.startsWith(`${rootPath}/`))
      : files.includes(rootPath);
    if (!present) throw new Error(`PliegoRS contract surface ${surface} is empty or missing`);
  }
  return [...new Set(files)];
}

function committedSurfaceSha256(root, revision) {
  const digest = createHash("sha256");
  for (const logical of committedSurfaceFiles(root, revision)) {
    const bytes = git(root, ["cat-file", "blob", `${revision}:${logical}`], null);
    digest.update(logical);
    digest.update("\0");
    digest.update(bytes);
    digest.update("\0");
  }
  return digest.digest("hex");
}

function dirtySurfaceStatus(root) {
  return git(
    root,
    [
      "status",
      "--porcelain=v1",
      "-z",
      "--untracked-files=all",
      "--",
      ...surfaceRoots(),
    ],
    null,
  );
}

export function verifyPliegorsContract(root, contractPath) {
  const contract = JSON.parse(readFileSync(contractPath, "utf8"));
  if (
    contract.schemaVersion !== 2 ||
    JSON.stringify(contract.surfaces) !==
      JSON.stringify(PLIEGORS_CONTRACT_SURFACES)
  ) {
    throw new Error("PliegoRS contract schema or canonical surfaces drifted");
  }
  const observed = inspectPliegorsContract(root);
  const { revision, sourceSha256 } = observed;
  if (revision !== contract.revision) {
    throw new Error(
      `PliegoRS contract revision drifted; expected ${contract.revision}, got ${revision}`,
    );
  }
  if (sourceSha256 !== contract.sourceSha256) {
    throw new Error(
      `PliegoRS contract source drifted; expected ${contract.sourceSha256}, got ${sourceSha256}`,
    );
  }
  return observed;
}

export function inspectPliegorsContract(root) {
  const revision = git(root, ["rev-parse", "HEAD"], "utf8").trim();
  const dirty = dirtySurfaceStatus(root);
  if (dirty.length > 0) {
    const status = dirty.toString("utf8").replaceAll("\0", "\n").trim();
    throw new Error(
      `PliegoRS contract surfaces have uncommitted changes; use a clean checkout:\n${status}`,
    );
  }
  return {
    revision,
    sourceSha256: committedSurfaceSha256(root, revision),
  };
}
