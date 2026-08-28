import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { isolatedCargoEnvironment } from "./rust-target.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const gates = Object.freeze([
  {
    id: "brand-system",
    tier: "fast",
    command: "pnpm",
    args: ["check:brand"],
  },
  {
    id: "rust-format",
    tier: "fast",
    command: "cargo",
    args: ["fmt", "--all", "--", "--check"],
  },
  {
    id: "rust-check",
    tier: "fast",
    command: "cargo",
    args: ["check", "--workspace", "--all-targets", "--locked"],
  },
  {
    id: "engine-boundary",
    tier: "fast",
    command: "pnpm",
    args: ["check:engine-boundary"],
  },
  {
    id: "benchmark-authority-v2",
    tier: "fast",
    command: "pnpm",
    args: ["check:benchmark-authority"],
  },
  {
    id: "browser-output-authority",
    tier: "fast",
    command: "pnpm",
    args: ["check:browser-output-authority"],
  },
  {
    id: "adapter-coexistence-authority",
    tier: "fast",
    command: "pnpm",
    args: ["check:adapter-coexistence-authority"],
  },
  {
    id: "external-adoption-authority",
    tier: "fast",
    command: "pnpm",
    args: ["check:external-adoption-authority"],
  },
  {
    id: "repository-distribution-contract",
    tier: "fast",
    command: "pnpm",
    args: ["check:repository-distribution"],
  },
  {
    id: "rust-tests",
    tier: "fast",
    command: "cargo",
    args: ["test", "--workspace", "--all-targets", "--locked"],
  },
  {
    id: "rust-doc-tests",
    tier: "fast",
    command: "cargo",
    args: ["test", "--workspace", "--doc", "--locked"],
  },
  {
    id: "rust-clippy",
    tier: "fast",
    command: "cargo",
    args: [
      "clippy",
      "--workspace",
      "--all-targets",
      "--locked",
      "--",
      "-D",
      "warnings",
    ],
  },
  {
    id: "docs",
    tier: "fast",
    command: "node",
    args: ["scripts/check-docs.mjs"],
  },
  {
    id: "maturity-map",
    tier: "fast",
    command: "node",
    args: ["scripts/check-maturity-map.mjs"],
  },
  {
    id: "tailwind-matrix",
    tier: "fast",
    command: "node",
    args: ["scripts/check-tailwind-matrix.mjs"],
  },
  {
    id: "representative-gap-map",
    tier: "fast",
    command: "node",
    args: ["scripts/check-representative-gap-map.mjs"],
  },
  {
    id: "standard-css-classifier-corpus",
    tier: "fast",
    command: "node",
    args: ["scripts/check-standard-css-classifier-corpus.mjs"],
  },
  {
    id: "accessibility-quality-corpus",
    tier: "fast",
    command: "cargo",
    args: [
      "test",
      "-p",
      "pliego-css-control",
      "--features",
      "projection",
      "--test",
      "accessibility_quality",
      "--locked",
    ],
  },
  {
    id: "generic-css-usage-contract",
    tier: "fast",
    command: "cargo",
    args: ["test", "-p", "pliego-css-usage", "--test", "generic_css_usage", "--locked"],
  },
  {
    id: "release-authority-contract-tests",
    tier: "fast",
    command: "node",
    args: ["scripts/release-authority.test.mjs"],
  },
  {
    id: "release-readiness-contract",
    tier: "fast",
    command: "node",
    args: ["scripts/check-release-readiness.mjs", "--validate"],
  },
  {
    id: "hosted-browser-matrix-contract",
    tier: "fast",
    command: "node",
    args: ["scripts/check-hosted-browser-matrix.mjs", "--validate"],
  },
  {
    id: "document-authority",
    tier: "fast",
    command: "node",
    args: ["scripts/check-document-authority.mjs"],
  },
  {
    id: "site-markdown-docs",
    tier: "fast",
    command: "pnpm",
    args: ["check:site-docs"],
  },
  {
    id: "getting-started",
    tier: "integration",
    command: "node",
    args: ["scripts/check-getting-started.mjs"],
  },
  {
    id: "benchmark-authority-smoke",
    tier: "integration",
    command: "pnpm",
    args: ["check:benchmark-smoke"],
  },
  {
    id: "standards-provenance",
    tier: "fast",
    command: "node",
    args: ["scripts/check-standards-provenance.mjs"],
  },
  {
    id: "compatibility",
    tier: "fast",
    command: "node",
    args: ["scripts/check-compatibility-policy.mjs"],
  },
  {
    id: "repair-corpus",
    tier: "fast",
    command: "node",
    args: ["scripts/check-repair-corpus.mjs"],
  },
  {
    id: "migration-corpus",
    tier: "fast",
    command: "node",
    args: ["scripts/check-migration-corpus.mjs"],
  },

  {
    id: "plain-html",
    tier: "integration",
    command: "node",
    args: ["scripts/check-plain-html-integration.mjs"],
  },
  {
    id: "representative-plain-html-audit",
    tier: "integration",
    command: "node",
    args: ["scripts/check-representative-plain-html-audit.mjs"],
  },
  {
    id: "representative-vite-tailwind",
    tier: "integration",
    command: "node",
    args: ["scripts/check-representative-vite-tailwind.mjs"],
  },
  {
    id: "representative-css-modules",
    tier: "integration",
    command: "node",
    args: ["scripts/check-representative-css-modules.mjs"],
  },
  {
    id: "representative-rust-control",
    tier: "integration",
    command: "node",
    args: ["scripts/check-representative-rust-control.mjs"],
  },
  {
    id: "representative-framework-routes",
    tier: "integration",
    command: "node",
    args: ["scripts/check-representative-framework-routes.mjs"],
  },
  {
    id: "adapter-coexistence",
    tier: "integration",
    command: "pnpm",
    args: ["check:adapter-coexistence"],
    requires: ["PLIEGOCSS_RUN_BROWSERS"],
  },
  {
    id: "lsp",
    tier: "integration",
    command: "node",
    args: ["scripts/check-lsp-integration.mjs"],
    rustToolchain: "1.85.0",
  },
  {
    id: "vscode",
    tier: "integration",
    command: "node",
    args: ["scripts/check-vscode-client.mjs"],
  },
  {
    id: "vscode-host",
    tier: "integration",
    command: "pnpm",
    args: ["--filter", "pliegocss-vscode", "test:host"],
    requires: ["PLIEGOCSS_RUN_EDITOR_HOSTS"],
    rustToolchain: "1.85.0",
  },
  {
    id: "neovim",
    tier: "integration",
    command: "node",
    args: ["scripts/check-neovim-client.mjs"],
    requires: ["PLIEGOCSS_RUN_EDITOR_HOSTS"],
    rustToolchain: "1.85.0",
  },
  {
    id: "pliegors-guide",
    tier: "integration",
    command: "node",
    args: ["scripts/check-pliegors-guide.mjs"],
  },
  {
    id: "pliegors",
    tier: "integration",
    command: "node",
    args: ["scripts/check-pliegors-integration.mjs"],
    requires: ["PLIEGORS_ROOT"],
  },
  {
    id: "pliegors-browser",
    tier: "integration",
    command: "node",
    args: ["scripts/check-pliegors-browser.mjs"],
    requires: ["PLIEGORS_ROOT", "PLIEGOCSS_RUN_BROWSERS"],
  },
  {
    id: "pliegors-dev",
    tier: "integration",
    command: "node",
    args: ["scripts/check-pliegors-dev-loop.mjs"],
    requires: ["PLIEGORS_ROOT", "PLIEGOCSS_RUN_BROWSERS"],
  },

  {
    id: "release-readiness",
    tier: "release",
    command: "node",
    args: ["scripts/check-release-readiness.mjs", "--profile=release"],
  },
  {
    id: "external-adoption",
    tier: "release",
    command: "pnpm",
    args: ["check:external-adoption"],
  },
  {
    id: "hosted-browser-matrix",
    tier: "release",
    command: "node",
    args: ["scripts/check-hosted-browser-matrix.mjs", "--profile=release"],
  },
  { id: "public-api", tier: "release", command: "pnpm", args: ["check:api"] },
  {
    id: "properties",
    tier: "release",
    command: "pnpm",
    args: ["check:properties"],
    rustToolchain: "1.85.0",
  },
  {
    id: "determinism",
    tier: "release",
    command: "pnpm",
    args: ["check:determinism"],
  },
  {
    id: "fuzz",
    tier: "release",
    command: "pnpm",
    args: ["check:fuzz"],
    requires: ["PLIEGOCSS_RUN_FUZZ"],
  },
  {
    id: "benchmark-evidence",
    tier: "release",
    command: "pnpm",
    args: ["check:evidence"],
  },
  {
    id: "benchmark-oracle-live",
    tier: "release",
    command: "pnpm",
    args: ["check:benchmark-oracle"],
  },
  {
    id: "media-query-merge",
    tier: "release",
    command: "pnpm",
    args: ["check:media"],
  },
  {
    id: "reachability-pruning",
    tier: "release",
    command: "pnpm",
    args: ["check:pruning"],
  },
  {
    id: "portability",
    tier: "release",
    command: "pnpm",
    args: ["check:portability"],
  },
  {
    id: "packages",
    tier: "release",
    command: "pnpm",
    args: ["check:packages"],
  },
  {
    id: "supply-chain",
    tier: "release",
    command: "pnpm",
    args: ["check:supply-chain"],
    requires: ["PLIEGOCSS_RUN_SUPPLY_CHAIN"],
  },
  {
    id: "site",
    tier: "release",
    command: "pnpm",
    args: ["check:site"],
  },
  {
    id: "site-deployment",
    tier: "release",
    command: "pnpm",
    args: ["check:site-deployment"],
  },
  {
    id: "migration-real-corpus",
    tier: "release",
    command: "pnpm",
    args: ["check:migration-real-corpus"],
    requires: ["PLIEGOCSS_RUN_NETWORK_CORPUS"],
  },
]);

const profileTiers = Object.freeze({
  fast: ["fast"],
  integration: ["fast", "integration"],
  release: ["fast", "integration", "release"],
});

function fail(message) {
  process.stderr.write(`${message}\n`);
  process.exitCode = 2;
}

function selectedGates(profile) {
  const includes = profileTiers[profile];
  if (!includes) return undefined;
  return {
    includes,
    gates: gates.filter((gate) => includes.includes(gate.tier)),
  };
}

function listedGate(gate) {
  return {
    id: gate.id,
    tier: gate.tier,
    command: [gate.command, ...gate.args],
    requires: gate.requires ?? [],
    rustToolchain: gate.rustToolchain ?? "active",
  };
}

function spawnGate(gate, environment) {
  if (process.platform === "win32" && gate.command === "pnpm") {
    return spawnSync(
      process.env.ComSpec ?? "cmd.exe",
      ["/d", "/s", "/c", gate.command, ...gate.args],
      {
        cwd: root,
        env: environment,
        encoding: "utf8",
        stdio: "inherit",
        windowsHide: true,
      },
    );
  }
  return spawnSync(gate.command, gate.args, {
    cwd: root,
    env: environment,
    encoding: "utf8",
    stdio: "inherit",
    windowsHide: true,
  });
}

function run(profile) {
  const selected = selectedGates(profile);
  if (!selected) {
    fail(`unknown verification profile: ${profile}`);
    return;
  }

  const results = [];
  const targetEnvironments = new Map();
  for (const gate of selected.gates) {
    const missing = (gate.requires ?? []).filter((name) => !process.env[name]);
    if (missing.length > 0) {
      results.push({ id: gate.id, status: "not-configured", missing });
      process.stdout.write(
        `[not-configured] ${gate.id}: ${missing.join(", ")}\n`,
      );
      continue;
    }
    const toolchain = gate.rustToolchain ?? "active";
    let environment = targetEnvironments.get(toolchain);
    if (!environment) {
      environment = isolatedCargoEnvironment(root, {
        env: process.env,
        toolchain: gate.rustToolchain,
      });
      targetEnvironments.set(toolchain, environment);
      process.stdout.write(`[target] ${toolchain}: ${environment.CARGO_TARGET_DIR}\n`);
    }
    process.stdout.write(`[running] ${gate.id}\n`);
    const result = spawnGate(gate, environment);
    if (result.error) {
      results.push({
        id: gate.id,
        status: "failed",
        error: result.error.message,
      });
      break;
    }
    if (result.status !== 0) {
      results.push({ id: gate.id, status: "failed", exitCode: result.status });
      break;
    }
    results.push({ id: gate.id, status: "passed" });
  }

  const failed = results.some((result) => result.status === "failed");
  const notConfigured = results.some(
    (result) => result.status === "not-configured",
  );
  const summary = {
    schemaVersion: 1,
    profile,
    result: failed ? "failed" : notConfigured ? "blocked" : "passed",
    results,
  };
  process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
  if (failed || (profile === "release" && notConfigured)) process.exitCode = 1;
}

const args = process.argv.slice(2);
if (args[0] === "--list") {
  const profile = args[1];
  const selected = selectedGates(profile);
  if (!selected) {
    fail(`unknown verification profile: ${profile}`);
  } else {
    process.stdout.write(
      `${JSON.stringify(
        {
          schemaVersion: 1,
          profile,
          includes: selected.includes,
          gates: selected.gates.map(listedGate),
        },
        null,
        2,
      )}\n`,
    );
  }
} else if (args.length === 1) {
  run(args[0]);
} else {
  fail(
    "usage: node scripts/check-release.mjs [--list] fast|integration|release",
  );
}
