import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const gates = Object.freeze([
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
    id: "release-readiness",
    tier: "fast",
    command: "node",
    args: ["scripts/check-release-readiness.mjs"],
  },
  {
    id: "hosted-browser-matrix",
    tier: "fast",
    command: "node",
    args: ["scripts/check-hosted-browser-matrix.mjs"],
  },
  {
    id: "getting-started",
    tier: "integration",
    command: "node",
    args: ["scripts/check-getting-started.mjs"],
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
    id: "lsp",
    tier: "integration",
    command: "node",
    args: ["scripts/check-lsp-integration.mjs"],
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
  },
  {
    id: "neovim",
    tier: "integration",
    command: "node",
    args: ["scripts/check-neovim-client.mjs"],
    requires: ["PLIEGOCSS_RUN_EDITOR_HOSTS"],
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

  { id: "public-api", tier: "release", command: "pnpm", args: ["check:api"] },
  {
    id: "properties",
    tier: "release",
    command: "pnpm",
    args: ["check:properties"],
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
  };
}

function run(profile) {
  const selected = selectedGates(profile);
  if (!selected) {
    fail(`unknown verification profile: ${profile}`);
    return;
  }

  const results = [];
  for (const gate of selected.gates) {
    const missing = (gate.requires ?? []).filter((name) => !process.env[name]);
    if (missing.length > 0) {
      results.push({ id: gate.id, status: "not-configured", missing });
      process.stdout.write(
        `[not-configured] ${gate.id}: ${missing.join(", ")}\n`,
      );
      continue;
    }
    process.stdout.write(`[running] ${gate.id}\n`);
    const result = spawnSync(gate.command, gate.args, {
      cwd: root,
      env: process.env,
      encoding: "utf8",
      stdio: "inherit",
      windowsHide: true,
    });
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
