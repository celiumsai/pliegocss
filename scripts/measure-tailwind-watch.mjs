import { spawn } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const benchmarkRoot = join(root, "benchmarks", "tailwind-v4");
const dist = join(benchmarkRoot, "dist");
const controlled = join(benchmarkRoot, "controlled");
const results = join(root, "benchmarks", "results");
const fixturePath = join(controlled, "watch", "index.html");
const outputPath = join(dist, "watch.css");
const cliPackage = JSON.parse(
  readFileSync(join(root, "node_modules", "@tailwindcss", "cli", "package.json"), "utf8"),
);
const tailwindCli = join(root, "node_modules", "@tailwindcss", "cli", cliPackage.bin.tailwindcss);
const original = readFileSync(join(benchmarkRoot, "fixtures.html"), "utf8");

mkdirSync(dist, { recursive: true });
mkdirSync(controlled, { recursive: true });
mkdirSync(dirname(fixturePath), { recursive: true });
mkdirSync(results, { recursive: true });
writeFileSync(fixturePath, original);

const child = spawn(
  process.execPath,
  [tailwindCli, "-i", join(benchmarkRoot, "input-watch.css"), "-o", outputPath, "--watch=always"],
  { cwd: root, stdio: ["ignore", "pipe", "pipe"] },
);

const waiters = [];
let buffer = "";
let fatal = "";

function consume(chunk) {
  buffer += chunk.toString("utf8");
  const lines = buffer.split(/\r?\n/u);
  buffer = lines.pop() ?? "";
  for (const rawLine of lines) {
    const line = rawLine.replace(/\u001B\[[0-9;]*m/gu, "");
    const match = line.match(/Done in\s+([0-9.]+)(ms|µs)/iu);
    if (match) {
      const value = Number(match[1]);
      waiters.shift()?.resolve(match[2].toLowerCase() === "ms" ? value : value / 1_000);
    }
  }
}

child.stdout.on("data", consume);
child.stderr.on("data", (chunk) => {
  fatal += chunk.toString("utf8");
  consume(chunk);
});

function nextBuild() {
  return new Promise((resolveBuild, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(`Timed out waiting for Tailwind watch rebuild.\n${fatal}`));
    }, 10_000);
    waiters.push({
      resolve(value) {
        clearTimeout(timer);
        resolveBuild(value);
      },
    });
  });
}

async function mutate(content) {
  const completed = nextBuild();
  const start = performance.now();
  writeFileSync(fixturePath, content);
  const engineMs = await completed;
  return { wallMs: performance.now() - start, engineMs };
}

function summarize(samples) {
  const metric = (name) => {
    const sorted = samples.map((sample) => sample[name]).sort((a, b) => a - b);
    const middle = Math.floor(sorted.length / 2);
    const median =
      sorted.length % 2 === 0
        ? (sorted[middle - 1] + sorted[middle]) / 2
        : sorted[middle];
    return {
      median: Number(median.toFixed(3)),
      p95: Number(sorted[Math.ceil(sorted.length * 0.95) - 1].toFixed(3)),
      min: Number(sorted[0].toFixed(3)),
      max: Number(sorted.at(-1).toFixed(3)),
    };
  };
  return { sampleCount: samples.length, wallMs: metric("wallMs"), engineMs: metric("engineMs") };
}

try {
  await nextBuild();

  for (let index = 0; index < 20; index += 1) {
    await mutate(`${original}\n<!-- warmup ${index} -->\n`);
  }

  const contentOnly = [];
  for (let index = 0; index < 100; index += 1) {
    contentOnly.push(await mutate(`${original}\n<!-- content mutation ${index} -->\n`));
  }

  const knownUtility = [];
  for (let index = 0; index < 100; index += 1) {
    knownUtility.push(await mutate(`${original}\n<div class="flex">${index}</div>\n`));
  }

  let growing = original;
  const newUtility = [];
  for (let index = 1; index <= 50; index += 1) {
    growing += `\n<div class="z-[${index}]">${index}</div>`;
    newUtility.push(await mutate(`${growing}\n`));
  }

  const report = {
    schemaVersion: 1,
    generatedAtUtc: new Date().toISOString(),
    tool: "tailwindcss-watch",
    version: cliPackage.version,
    profiles: {
      contentOnly: summarize(contentOnly),
      additionalKnownUtility: summarize(knownUtility),
      newUtility: summarize(newUtility),
    },
    caveat: "Wall time includes filesystem notification and CLI reporting. Engine time is parsed from Tailwind's own completion line.",
  };
  writeFileSync(join(results, "tailwind-watch.local.json"), `${JSON.stringify(report, null, 2)}\n`);
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
} finally {
  child.kill();
  rmSync(join(controlled, "watch"), { recursive: true, force: true });
}
