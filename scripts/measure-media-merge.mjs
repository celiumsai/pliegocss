import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

const scriptPath = fileURLToPath(import.meta.url);
const root = resolve(dirname(scriptPath), "..");
const fixturePath = join(root, "benchmarks", "media-merge", "adjacent-md.styles.txt");
const gateAPath = join(root, "benchmarks", "pliego-gate-a", "core-fixture.html");
const gateBPath = join(root, "benchmarks", "tailwind-v4", "fixtures.html");
const resultPath = join(root, "benchmarks", "results", "media-merge.local.json");
const runtime = join(root, "target", "benchmarks", "media-merge", `run-${process.pid}`);
const buildTarget = join(root, "target", "benchmarks", "media-merge", "build");
const check = process.argv.length === 3 && process.argv[2] === "--check";
const expected = {
  fixtureSha256: "b7b0b17af67c09ace83bb95ac06c44afa4ae87e1fa5b206f2fef00fea3f0fc06",
  profiles: {
    utilitiesOnly: {
      control: {
        sha256: "efee258d711879f4e7c89e134c88c9c797d65e524727878a72e474894f17d2c2",
        rawBytes: 1_449,
        gzipBytes: 649,
        mediaWrappers: 20,
      },
      candidate: {
        sha256: "ac316f5581edfef5562ade30dc758815ab724665cade8b42ea748349d293c5f1",
        rawBytes: 1_012,
        gzipBytes: 637,
        mediaWrappers: 1,
      },
    },
    themeAndUtilities: {
      control: {
        sha256: "908dbf7df76c4598f89bf9bcf9c6b3b91918ca964275a6e1536d11419d31d608",
        rawBytes: 1_820,
        gzipBytes: 821,
        mediaWrappers: 20,
      },
      candidate: {
        sha256: "05b254d481de13dd11d235308e5d8bf3dc977829dec3e71f1fc042b2f7793f45",
        rawBytes: 1_383,
        gzipBytes: 809,
        mediaWrappers: 1,
      },
    },
  },
  neutralFixtures: {
    gateA: {
      sha256: "1d879683af938faba7a7e44a0ad330e9f2f6a5f6b363b0f1556cc56711cd6f5b",
      rawBytes: 6_548,
      gzipBytes: 1_596,
      mediaWrappers: 0,
    },
    gateB: {
      sha256: "757e259d072723eb06ea5517627ac3422e73c16f5eb36b6ecc57c8aa104c126d",
      rawBytes: 10_461,
      gzipBytes: 2_046,
      mediaWrappers: 12,
    },
  },
};

if (!check && process.argv.length !== 2) {
  throw new Error("usage: node scripts/measure-media-merge.mjs [--check]");
}

function fail(message) {
  throw new Error(message);
}

function assert(condition, message) {
  if (!condition) fail(message);
}

function assertEqual(actual, expectedValue, message) {
  if (JSON.stringify(actual) !== JSON.stringify(expectedValue)) {
    fail(`${message}\nactual: ${JSON.stringify(actual)}\nexpected: ${JSON.stringify(expectedValue)}`);
  }
}

function run(program, args, options = {}) {
  const result = spawnSync(program, args, {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
    timeout: 180_000,
    windowsHide: true,
    ...options,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    fail(
      `${program} ${args.join(" ")} exited ${result.status}\n` +
        `stdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
    );
  }
  return result;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function resolveExecutable() {
  if (process.env.PLIEGO_CSSC) {
    const configured = resolve(root, process.env.PLIEGO_CSSC);
    assert(existsSync(configured), `PLIEGO_CSSC does not exist: ${configured}`);
    return configured;
  }
  run("cargo", ["+1.96.0", "build", "--locked", "-p", "pliego-cssc"], {
    env: { ...process.env, CARGO_TARGET_DIR: buildTarget, CARGO_INCREMENTAL: "0" },
  });
  const executable = join(
    buildTarget,
    "debug",
    process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc",
  );
  assert(existsSync(executable), `cargo did not produce ${executable}`);
  return executable;
}

function compile(executable, styles, { theme = false, manifest } = {}) {
  const args = [
    "compile",
    "--seed",
    "--targets",
    "modern",
    "--format",
    "minified",
    ...(theme ? ["--theme"] : []),
    ...styles.flatMap((style) => ["--style", style]),
    ...(manifest ? ["--manifest", manifest] : []),
  ];
  const result = run(executable, args);
  assert(result.stderr === "", `successful compile emitted stderr:\n${result.stderr}`);
  assert(result.stdout.endsWith("\n"), "compiler CSS lost its final newline");
  return result.stdout;
}

function unwrapMedia(css) {
  const text = css.slice(0, -1);
  assert(text.startsWith("@media ") && text.endsWith("}"), "control is not one media rule");
  const open = text.indexOf("{");
  assert(open > "@media ".length, "control media rule has no query");
  return { query: text.slice("@media ".length, open), body: text.slice(open + 1, -1) };
}

function measurements(css) {
  const bytes = Buffer.from(css);
  return {
    sha256: sha256(bytes),
    rawBytes: bytes.length,
    gzipBytes: gzipSync(bytes, { level: 9 }).length,
    mediaWrappers: (css.match(/@media/gu) ?? []).length,
  };
}

function profile(control, candidate) {
  const controlMetrics = measurements(control);
  const candidateMetrics = measurements(candidate);
  const rawSaved = controlMetrics.rawBytes - candidateMetrics.rawBytes;
  const gzipSaved = controlMetrics.gzipBytes - candidateMetrics.gzipBytes;
  return {
    control: controlMetrics,
    candidate: candidateMetrics,
    delta: {
      rawBytesSaved: rawSaved,
      rawPercentSaved: Number(((rawSaved / controlMetrics.rawBytes) * 100).toFixed(3)),
      gzipBytesSaved: gzipSaved,
      gzipPercentSaved: Number(((gzipSaved / controlMetrics.gzipBytes) * 100).toFixed(3)),
    },
  };
}

process.once("exit", () => rmSync(runtime, { recursive: true, force: true }));
rmSync(runtime, { recursive: true, force: true });
mkdirSync(runtime, { recursive: true });

const fixtureBytes = readFileSync(fixturePath);
assert(sha256(fixtureBytes) === expected.fixtureSha256, "media merge fixture hash drifted");
const styles = fixtureBytes
  .toString("utf8")
  .split(/\r?\n/u)
  .map((line) => line.trim())
  .filter(Boolean);
assert(styles.length === 20 && new Set(styles).size === styles.length, "fixture must contain 20 unique styles");

const executable = resolveExecutable();
const manifestPath = join(runtime, "combined.manifest.json");
const candidateUtilities = compile(executable, styles, { manifest: manifestPath });
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
const orderedStyles = manifest.styles.map((style) => style.origins[0]?.source);
assert(
  orderedStyles.length === styles.length &&
    new Set(orderedStyles).size === styles.length &&
    orderedStyles.every((style) => styles.includes(style)),
  "manifest did not retain every fixture style exactly once",
);

const atomic = orderedStyles.map((style) => compile(executable, [style]));
const media = atomic.map(unwrapMedia);
assert(new Set(media.map((rule) => rule.query)).size === 1, "fixture media queries diverged");
const controlUtilities = `${media.map((rule) => `@media ${rule.query}{${rule.body}}`).join("")}\n`;
const expectedCandidate = `@media ${media[0].query}{${media.map((rule) => rule.body).join("")}}\n`;
assert(candidateUtilities === expectedCandidate, "candidate changed rule order or declarations");
assert(compile(executable, styles) === candidateUtilities, "candidate output is not deterministic");

const firstWithTheme = compile(executable, [orderedStyles[0]], { theme: true });
assert(firstWithTheme.endsWith(atomic[0]), "theme prefix is not independent from utility CSS");
const themePrefix = firstWithTheme.slice(0, -atomic[0].length);
const controlTheme = `${themePrefix}${controlUtilities}`;
const candidateTheme = compile(executable, styles, { theme: true });
assert(candidateTheme === `${themePrefix}${candidateUtilities}`, "theme changed merged utility bytes");

const utilitiesOnly = profile(controlUtilities, candidateUtilities);
const themeAndUtilities = profile(controlTheme, candidateTheme);
if (process.env.PLIEGO_SKIP_GZIP_HASH !== "1") {
  assertEqual(
    {
      utilitiesOnly: {
        control: utilitiesOnly.control,
        candidate: utilitiesOnly.candidate,
      },
      themeAndUtilities: {
        control: themeAndUtilities.control,
        candidate: themeAndUtilities.candidate,
      },
    },
    expected.profiles,
    "reviewed media merge size/hash contract drifted",
  );
}

const gateABytes = readFileSync(gateAPath);
const gateAStyles = [...gateABytes.toString("utf8").matchAll(/class="([^"]*)"/gu)].map(
  (match) => match[1],
);
const gateBBytes = readFileSync(gateBPath);
const gateBStyles = [...gateBBytes.toString("utf8").matchAll(/class="([^"]*)"/gu)].map(
  (match) => match[1],
);
assert(gateAStyles.length === 44, "Gate A class-attribute count drifted");
assert(gateBStyles.length === 44, "Gate B class-attribute count drifted");
const neutralFixtures = {
  gateA: measurements(compile(executable, gateAStyles, { theme: true })),
  gateB: measurements(compile(executable, gateBStyles, { theme: true })),
};
assertEqual(neutralFixtures, expected.neutralFixtures, "Gate A/B neutral CSS contract drifted");

const gates = {
  exactRuleSequence: candidateUtilities === expectedCandidate,
  utilitiesRawReduction: utilitiesOnly.delta.rawBytesSaved > 0,
  themeRawReduction: themeAndUtilities.delta.rawBytesSaved > 0,
  themeGzipMinimumBytes: themeAndUtilities.delta.gzipBytesSaved >= 8,
  themeGzipMinimumPercent:
    themeAndUtilities.delta.gzipBytesSaved * 100 >= themeAndUtilities.control.gzipBytes,
  wrapperReduction:
    utilitiesOnly.control.mediaWrappers === styles.length &&
    utilitiesOnly.candidate.mediaWrappers === 1,
};
assert(Object.values(gates).every(Boolean), `media merge adoption gate failed: ${JSON.stringify(gates)}`);

const git = run("git", ["rev-parse", "HEAD"]).stdout.trim();
const dirty = run("git", ["status", "--porcelain=v1", "--untracked-files=all"]).stdout.length > 0;
const report = {
  schemaVersion: 1,
  claimScope: "targeted-responsive-only",
  generatedAt: new Date().toISOString(),
  git: { commit: git, dirty },
  runtime: { node: process.version, zlib: process.versions.zlib },
  inputs: {
    fixture: "benchmarks/media-merge/adjacent-md.styles.txt",
    fixtureSha256: sha256(fixtureBytes),
    harness: "scripts/measure-media-merge.mjs",
    harnessSha256: sha256(readFileSync(scriptPath)),
    compilerSha256: sha256(readFileSync(executable)),
    compilerSource: process.env.PLIEGO_CSSC ? "PLIEGO_CSSC-prebuilt" : "cargo-1.96.0",
    gateAFixtureSha256: sha256(gateABytes),
    gateBFixtureSha256: sha256(gateBBytes),
    styles: styles.length,
  },
  profiles: { utilitiesOnly, themeAndUtilities },
  neutralFixtures,
  gates,
};

if (!check) {
  mkdirSync(dirname(resultPath), { recursive: true });
  writeFileSync(resultPath, `${JSON.stringify(report, null, 2)}\n`);
}
console.log(JSON.stringify(report, null, 2));
