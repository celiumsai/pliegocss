// SPDX-License-Identifier: Apache-2.0

import { spawn, spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";

const siteRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const root = resolve(siteRoot, "..");
const wrangler = resolve(
  siteRoot,
  "node_modules",
  "wrangler",
  "bin",
  "wrangler.js",
);

function fail(message) {
  throw new Error(`site deployment contract: ${message}`);
}

function runWrangler(args, options = {}) {
  const command = process.execPath;
  const commandArgs = [wrangler, ...args];
  return options.spawn
    ? spawn(command, commandArgs, {
        cwd: siteRoot,
        env: { ...process.env, NO_UPDATE_NOTIFIER: "1" },
        stdio: options.stdio ?? "pipe",
        windowsHide: true,
        detached: process.platform !== "win32",
      })
    : spawnSync(command, commandArgs, {
        cwd: siteRoot,
        encoding: "utf8",
        env: { ...process.env, NO_UPDATE_NOTIFIER: "1" },
        stdio: ["ignore", "pipe", "pipe"],
        windowsHide: true,
        timeout: options.timeout ?? 120_000,
      });
}

async function freePort() {
  const server = createServer();
  await new Promise((resolveListen, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolveListen);
  });
  const address = server.address();
  const port = typeof address === "object" && address ? address.port : undefined;
  await new Promise((resolveClose) => server.close(resolveClose));
  if (!port) fail("could not reserve a local port");
  return port;
}

function assertConfig() {
  const config = JSON.parse(
    readFileSync(join(siteRoot, "wrangler.jsonc"), "utf8"),
  );
  if (
    config.name !== "pliegocss-site" ||
    config.workers_dev !== false ||
    config.preview_urls !== false ||
    config.assets?.directory !== "./target/site" ||
    config.assets?.html_handling !== "auto-trailing-slash" ||
    config.assets?.not_found_handling !== "404-page" ||
    config.routes?.length !== 1 ||
    config.routes[0]?.pattern !== "pliegocss.dev" ||
    config.routes[0]?.custom_domain !== true ||
    config.routes[0]?.previews_enabled !== false
  ) {
    fail("public Cloudflare profile drifted");
  }
}

function assertHeader(response, name, expected) {
  const actual = response.headers.get(name);
  if (!actual?.includes(expected)) {
    fail(`${name} for ${response.url} does not include ${expected}: ${actual}`);
  }
}

function localFetch(url, options = {}) {
  return fetch(url, {
    ...options,
    signal: AbortSignal.timeout(5_000),
  });
}

async function responseContract(origin) {
  const home = await localFetch(`${origin}/`, { redirect: "manual" });
  if (home.status !== 200) fail(`home returned ${home.status}`);
  assertHeader(home, "content-security-policy", "frame-ancestors 'none'");
  assertHeader(
    home,
    "content-security-policy",
    "sha256-P+hnoNLWrWxk0w21ITjEpd7UnKmafsG79zKxPk/e3t4=",
  );
  assertHeader(home, "strict-transport-security", "max-age=63072000");
  assertHeader(home, "x-content-type-options", "nosniff");
  assertHeader(home, "cache-control", "max-age=0");

  const docsRedirect = await localFetch(`${origin}/docs`, {
    redirect: "manual",
  });
  if (
    ![301, 307, 308].includes(docsRedirect.status) ||
    !docsRedirect.headers.get("location")?.endsWith("/docs/")
  ) {
    fail(`docs trailing-slash redirect drifted (${docsRedirect.status})`);
  }
  const docs = await localFetch(`${origin}/docs/`);
  if (docs.status !== 200) fail(`docs returned ${docs.status}`);

  const asset = await localFetch(`${origin}/assets/site.js`);
  if (asset.status !== 200) fail(`site.js returned ${asset.status}`);
  assertHeader(asset, "cache-control", "max-age=31536000");
  assertHeader(asset, "cache-control", "immutable");

  const security = await localFetch(`${origin}/.well-known/security.txt`);
  if (security.status !== 200) fail(`security.txt returned ${security.status}`);
  assertHeader(security, "content-type", "text/plain");

  const missing = await localFetch(`${origin}/missing-contract-route`, {
    redirect: "manual",
  });
  if (missing.status !== 404) fail(`missing route returned ${missing.status}`);
}

function waitForExit(child, timeoutMs) {
  if (child.exitCode !== null) return Promise.resolve(true);
  return Promise.race([
    new Promise((resolveExit) => child.once("exit", () => resolveExit(true))),
    delay(timeoutMs).then(() => false),
  ]);
}

async function terminateProcessTree(child) {
  if (child.exitCode !== null) return;
  if (!child.pid) fail("Wrangler dev has no process id");
  if (process.platform === "win32") {
    const terminated = spawnSync(
      "taskkill.exe",
      ["/pid", String(child.pid), "/t", "/f"],
      {
        stdio: "ignore",
        windowsHide: true,
        timeout: 10_000,
      },
    );
    if (terminated.error && terminated.error.code !== "ESRCH") {
      throw terminated.error;
    }
  } else {
    try {
      process.kill(-child.pid, "SIGTERM");
    } catch (error) {
      if (error?.code !== "ESRCH") throw error;
    }
  }
  if (await waitForExit(child, 5_000)) return;
  if (process.platform !== "win32") {
    try {
      process.kill(-child.pid, "SIGKILL");
    } catch (error) {
      if (error?.code !== "ESRCH") throw error;
    }
  }
  if (!(await waitForExit(child, 5_000))) {
    fail(`Wrangler process tree ${child.pid} did not terminate`);
  }
}

assertConfig();

const outdir = mkdtempSync(join(tmpdir(), "pliegocss-wrangler-dry-run-"));
try {
  const dryRun = runWrangler(["deploy", "--dry-run", "--outdir", outdir]);
  if (dryRun.error) throw dryRun.error;
  if (dryRun.status !== 0) {
    fail(`Wrangler dry run failed\n${dryRun.stdout}\n${dryRun.stderr}`);
  }
} finally {
  rmSync(outdir, { force: true, recursive: true });
}

const port = await freePort();
const child = runWrangler(
  ["dev", "--local", "--ip", "127.0.0.1", "--port", String(port)],
  { spawn: true },
);
let stdout = "";
let stderr = "";
let spawnError;
child.stdout?.on("data", (chunk) => {
  stdout += chunk;
});
child.stderr?.on("data", (chunk) => {
  stderr += chunk;
});
child.once("error", (error) => {
  spawnError = error;
});

try {
  const origin = `http://127.0.0.1:${port}`;
  let ready = false;
  for (let attempt = 0; attempt < 120; attempt += 1) {
    if (spawnError || child.exitCode !== null) break;
    try {
      const response = await localFetch(`${origin}/`);
      if (response.ok) {
        ready = true;
        break;
      }
    } catch {
      // Wrangler has not bound the local socket yet.
    }
    await delay(100);
  }
  if (spawnError) throw spawnError;
  if (!ready) fail(`Wrangler dev did not start\n${stdout}\n${stderr}`);
  await responseContract(origin);
} finally {
  await terminateProcessTree(child);
}

process.stdout.write(
  `${JSON.stringify(
    {
      schemaVersion: 1,
      result: "passed",
      target: "Cloudflare Workers Static Assets",
      hostname: "pliegocss.dev",
      deployment: "not-performed",
      wrangler: "4.110.0",
    },
    null,
    2,
  )}\n`,
);
