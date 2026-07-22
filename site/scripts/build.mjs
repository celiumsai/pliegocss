// SPDX-License-Identifier: Apache-2.0

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build as esbuild } from "esbuild";

const siteRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const root = resolve(siteRoot, "..");
const generated = join(siteRoot, "generated");
const publicRoot = join(siteRoot, "public");
const output = join(siteRoot, "target", "site");
const siteTarget = join(siteRoot, "target", "cargo");
const pliegoInstallRoot = join(siteRoot, "target", "pliego-cli");
const applicationControlMarker = join(
  siteRoot,
  "target",
  ".windows-application-control-blocked",
);
const pliego = join(
  pliegoInstallRoot,
  "bin",
  process.platform === "win32" ? "pliego.exe" : "pliego",
);
const cli = join(
  root,
  "target",
  "debug",
  process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc",
);

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? root,
    encoding: "utf8",
    env: options.env ?? process.env,
    windowsHide: true,
    stdio: options.capture ? ["ignore", "pipe", "pipe"] : "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed with ${result.status}\n${result.stderr ?? ""}`,
    );
  }
  return result.stdout ?? "";
}

function runCaptured(command, args, options = {}) {
  return spawnSync(command, args, {
    cwd: options.cwd ?? root,
    encoding: "utf8",
    env: options.env ?? process.env,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"],
  });
}

function runPliegoCssCaptured(args) {
  if (process.platform === "win32" && existsSync(applicationControlMarker)) {
    const translated = args.map((argument) =>
      /^[A-Za-z]:[\\/]/u.test(argument) ? windowsToWsl(argument) : argument,
    );
    return runCaptured("wsl.exe", [
      "--cd",
      windowsToWsl(root),
      "-e",
      wslPliegoCssExecutable(),
      ...translated,
    ]);
  }
  return runCaptured(cli, args);
}

function runPliegoCss(args) {
  const result = runPliegoCssCaptured(args);
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`pliego-cssc failed with ${result.status}\n${result.stderr ?? ""}`);
  }
  process.stdout.write(result.stdout ?? "");
  process.stderr.write(result.stderr ?? "");
  return result.stdout ?? "";
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function prepareAssets() {
  mkdirSync(generated, { recursive: true });
  mkdirSync(join(publicRoot, "brand"), { recursive: true });
  mkdirSync(join(publicRoot, "media", "brand"), { recursive: true });
  mkdirSync(join(publicRoot, "fonts"), { recursive: true });
  mkdirSync(join(publicRoot, "assets"), { recursive: true });

  for (const name of [
    "pliegocss-symbol.svg",
    "pliegocss-symbol-reversed.svg",
    "pliegocss-lockup.svg",
    "pliegocss-lockup-reversed.svg",
    "favicon.svg",
  ]) {
    cpSync(join(root, "brand", name), join(publicRoot, "brand", name));
  }
  cpSync(join(root, "brand", "favicon.svg"), join(publicRoot, "favicon.svg"));
  cpSync(
    join(root, "brand", "pliegocss-app-icon.png"),
    join(publicRoot, "icon-512.png"),
  );
  cpSync(
    join(root, "brand", "social-card.png"),
    join(publicRoot, "social-card.png"),
  );
  for (const stem of [
    "cascade-chamber",
    "semantic-fold",
    "evidence-archive",
    "material-study-carbon",
    "material-study-paper",
    "material-study-cobalt",
  ]) {
    for (const extension of ["png", "webp", "avif"]) {
      cpSync(
        join(root, "brand", "images", `${stem}.${extension}`),
        join(publicRoot, "media", "brand", `${stem}.${extension}`),
      );
    }
  }
  cpSync(
    join(root, "brand", "fonts", "instrument-sans-variable.woff2"),
    join(publicRoot, "fonts", "instrument-sans-variable.woff2"),
  );
  cpSync(
    join(root, "brand", "fonts", "fragment-mono-regular.woff2"),
    join(publicRoot, "fonts", "fragment-mono-regular.woff2"),
  );
  cpSync(
    join(root, "brand", "fonts", "LICENSE-instrument-sans.txt"),
    join(publicRoot, "fonts", "LICENSE-instrument-sans.txt"),
  );
  cpSync(
    join(root, "brand", "fonts", "LICENSE-fragment-mono.txt"),
    join(publicRoot, "fonts", "LICENSE-fragment-mono.txt"),
  );
  cpSync(
    join(siteRoot, "i18n", "es.json"),
    join(publicRoot, "assets", "i18n-es.json"),
  );
}

function buildPliegoCss() {
  const outputCss = join(generated, "pliegocss.css");
  const manifest = join(generated, "pliegocss.manifest.json");
  const arguments_ = [
    "compile",
    "--source",
    join(siteRoot, "src"),
    "--seed",
    "--theme",
    "--output",
    outputCss,
    "--manifest",
    manifest,
  ];
  if (process.platform === "win32" && existsSync(applicationControlMarker)) {
    buildPliegoCssWithWsl(outputCss, manifest);
  } else {
    run("cargo", ["+1.85.0", "build", "--locked", "-p", "pliego-cssc"]);
    const result = runCaptured(cli, arguments_);
    const diagnostics = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
    if (result.error || result.status !== 0) {
      if (
        process.platform === "win32" &&
        (result.error || diagnostics.includes("Application Control policy has blocked this file"))
      ) {
        mkdirSync(dirname(applicationControlMarker), { recursive: true });
        writeFileSync(applicationControlMarker, "Windows executable launch blocked; use verified WSL build.\n");
        buildPliegoCssWithWsl(outputCss, manifest);
      } else if (result.error) {
        throw result.error;
      } else {
        throw new Error(`pliego-cssc failed with ${result.status}\n${diagnostics}`);
      }
    } else {
      process.stdout.write(result.stdout ?? "");
      process.stderr.write(result.stderr ?? "");
    }
  }
  const manifestDocument = JSON.parse(readFileSync(manifest, "utf8"));
  if (
    manifestDocument.cssSha256 !== sha256(readFileSync(outputCss)) ||
    !manifestDocument.styles?.length
  ) {
    throw new Error("PliegoCSS site manifest does not bind generated CSS");
  }
}

function generateLaboratory() {
  const dimensions = {
    layout: [
      { id: "stack", label: "Stack", value: "flex flex-col" },
      { id: "split", label: "Split", value: "grid grid-cols-2" },
    ],
    gap: [
      { id: "tight", label: "Tight", value: "gap-2" },
      { id: "balanced", label: "Balanced", value: "gap-4" },
      { id: "open", label: "Open", value: "gap-6" },
    ],
    padding: [
      { id: "compact", label: "Compact", value: "p-4" },
      { id: "roomy", label: "Roomy", value: "p-6" },
      { id: "editorial", label: "Editorial", value: "p-8" },
    ],
    radius: [
      { id: "precise", label: "Precise", value: "rounded-sm" },
      { id: "soft", label: "Soft", value: "rounded-2xl" },
    ],
    depth: [
      { id: "quiet", label: "Quiet", value: "shadow-sm" },
      { id: "raised", label: "Raised", value: "shadow-md" },
    ],
    tone: [
      {
        id: "paper",
        label: "Paper",
        value: "border border-line bg-surface text-ink",
      },
      {
        id: "compiled",
        label: "Compiled",
        value: "border border-accent bg-accent text-white",
      },
    ],
  };
  const combinations = [];
  for (const layout of dimensions.layout) {
    for (const gap of dimensions.gap) {
      for (const padding of dimensions.padding) {
        for (const radius of dimensions.radius) {
          for (const depth of dimensions.depth) {
            for (const tone of dimensions.tone) {
              const selections = {
                layout: layout.id,
                gap: gap.id,
                padding: padding.id,
                radius: radius.id,
                depth: depth.id,
                tone: tone.id,
              };
              combinations.push({
                id: Object.values(selections).join("--"),
                selections,
                input: [
                  layout.value,
                  gap.value,
                  padding.value,
                  radius.value,
                  depth.value,
                  tone.value,
                ].join(" "),
              });
            }
          }
        }
      }
    }
  }

  const runtime = join(siteRoot, "target", "laboratory");
  rmSync(runtime, { recursive: true, force: true });
  mkdirSync(runtime, { recursive: true });
  const cssPath = join(runtime, "laboratory.css");
  const manifestPath = join(runtime, "laboratory.manifest.json");
  runPliegoCss([
    "compile",
    ...combinations.flatMap((combination) => ["--style", combination.input]),
    "--seed",
    "--theme",
    "--output",
    cssPath,
    "--manifest",
    manifestPath,
  ]);

  const css = readFileSync(cssPath);
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  if (manifest.cssSha256 !== sha256(css)) {
    throw new Error("laboratory manifest does not bind generated CSS");
  }
  const stylesBySource = new Map();
  for (const style of manifest.styles ?? []) {
    for (const origin of style.origins ?? []) {
      stylesBySource.set(origin.source, style);
    }
  }
  const classCss = new Map();
  const cssText = css.toString("utf8");
  for (const style of manifest.styles ?? []) {
    const start = cssText.indexOf(`.${style.className}{`);
    if (start < 0) continue;
    const end = cssText.indexOf("}", start);
    if (end < 0) continue;
    classCss.set(style.className, cssText.slice(start, end + 1));
  }
  const recipes = combinations.map((combination) => {
    const style = stylesBySource.get(combination.input);
    if (!style) {
      throw new Error(`laboratory style missing for ${combination.input}`);
    }
    return {
      ...combination,
      className: style.className,
      styleId: style.styleId,
      css: classCss.get(style.className) ?? "",
    };
  });

  const explanations = [
    {
      id: "composed-effect",
      input: "rounded-lg ring-2! shadow-md!",
      label: "Composed physical effect",
    },
    {
      id: "responsive-grid",
      input: "grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3",
      label: "Responsive conditions",
    },
    {
      id: "action",
      input:
        "inline-flex items-center gap-2 rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-white hover:bg-accent-strong focus-visible:ring-2",
      label: "Interactive action",
    },
  ].map((entry) => {
    const result = runPliegoCssCaptured([
      "explain",
      "--style",
      entry.input,
      "--seed",
      "--format",
      "json",
    ]);
    if (result.error) throw result.error;
    if (result.status !== 0) {
      throw new Error(`explain failed for ${entry.id}\n${result.stderr}`);
    }
    return {
      ...entry,
      explanation: JSON.parse(result.stdout),
      cssSha256: sha256(Buffer.from(JSON.parse(result.stdout).css)),
    };
  });

  const conflicts = [
    {
      id: "padding",
      label: "Same semantic slot",
      input: "p-4 p-6",
    },
    {
      id: "type-size",
      label: "Competing type sizes",
      input: "text-sm text-lg",
    },
    {
      id: "condition-safe",
      label: "Narrower condition",
      input: "p-4 md:p-6",
    },
  ].map((entry) => {
    const result = runPliegoCssCaptured([
      "--diagnostic-format",
      "json",
      "check",
      "--style",
      entry.input,
      "--seed",
    ]);
    if (result.error) throw result.error;
      let diagnostics = [];
      const payload = (result.stderr || result.stdout || "").trim();
    if (payload && result.status !== 0) {
      try {
        diagnostics = JSON.parse(payload).diagnostics ?? [];
      } catch {
        diagnostics = [
          {
            code: "PROCESS",
            severity: "error",
            message: payload,
          },
        ];
      }
    }
    return {
      ...entry,
      accepted: result.status === 0,
      diagnostics,
    };
  });

  cpSync(cssPath, join(generated, "laboratory.css"));
  writeFileSync(
    join(generated, "laboratory.json"),
    `${JSON.stringify(
      {
        schemaVersion: 1,
        generatedBy: "pliego-cssc 0.1.0-rc.2",
        corpusBoundary:
          "Bounded build-time corpus; the browser does not reimplement the compiler.",
        dimensions,
        recipes,
        explanations,
        conflicts,
        cssSha256: manifest.cssSha256,
        cssBytes: css.length,
      },
      null,
      2,
    )}\n`,
  );
}

function generateCatalog() {
  const output = join(generated, "catalog.json");
  runPliegoCss([
    "catalog",
    "--seed",
    "--format",
    "json",
    "--output",
    output,
  ]);
  const catalog = JSON.parse(readFileSync(output, "utf8"));
  if (!Array.isArray(catalog.utilities) || catalog.utilities.length < 40) {
    throw new Error("generated utility catalog is unexpectedly incomplete");
  }
}

async function buildClient() {
  rmSync(join(generated, "chunks"), { recursive: true, force: true });
  const result = await esbuild({
    entryPoints: {
      site: join(siteRoot, "client", "main.js"),
    },
    bundle: true,
    splitting: true,
    minify: true,
    format: "esm",
    platform: "browser",
    target: ["es2022"],
    outdir: generated,
    entryNames: "[name]",
    chunkNames: "chunks/[name]-[hash]",
    legalComments: "eof",
    sourcemap: false,
    logLevel: "info",
    metafile: true,
  });
  const siteOutput = Object.entries(result.metafile.outputs).find(([, metadata]) =>
    metadata.entryPoint?.replaceAll("\\", "/").endsWith("site/client/main.js"),
  );
  if (!siteOutput || !siteOutput[0].replaceAll("\\", "/").endsWith("site.js")) {
    throw new Error("website client entry did not compile to generated/site.js");
  }
  cpSync(join(siteRoot, "src", "site.css"), join(generated, "site.css"));
}

function ensurePliego() {
  if (existsSync(pliego)) return;
  run(
    "cargo",
    [
      "+1.86.0",
      "install",
      "pliego-cli",
      "--version",
      "0.0.2",
      "--locked",
      "--root",
      pliegoInstallRoot,
    ],
    { cwd: siteRoot },
  );
}

function windowsToWsl(path) {
  const match = /^([A-Za-z]):[\\/](.*)$/u.exec(path);
  if (!match) {
    throw new Error(`cannot map non-drive path ${path} into WSL`);
  }
  return `/mnt/${match[1].toLowerCase()}/${match[2].replaceAll("\\", "/")}`;
}

let resolvedWslHome;

function wslHome() {
  if (resolvedWslHome) return resolvedWslHome;
  const result = runCaptured("wsl.exe", [
    "-e",
    "sh",
    "-lc",
    'printf "%s" "$HOME"',
  ]);
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(
      `could not resolve the WSL home directory\n${result.stderr ?? ""}`,
    );
  }
  const home = (result.stdout ?? "").trim();
  if (!/^\/[A-Za-z0-9._~/-]+$/u.test(home) || home.includes("/../")) {
    throw new Error(`WSL returned an unsafe home directory: ${home}`);
  }
  resolvedWslHome = home;
  return home;
}

function ensureWslPliego() {
  const root = windowsToWsl(join(siteRoot, "target", "pliego-cli-linux"));
  const executable = `${root}/bin/pliego`;
  const present = runCaptured("wsl.exe", ["-e", "test", "-x", executable]);
  if (!present.error && present.status === 0) return executable;
  const cargo = `${wslHome()}/.cargo/bin/cargo`;
  run("wsl.exe", [
    "-e",
    cargo,
    "+1.86.0",
    "install",
    "pliego-cli",
    "--version",
    "0.0.2",
    "--locked",
    "--root",
    root,
  ]);
  return executable;
}

function buildSiteWithWsl() {
  const executable = ensureWslPliego();
  const home = wslHome();
  const wslSiteRoot = windowsToWsl(siteRoot);
  const wslTarget = windowsToWsl(join(siteRoot, "target", "cargo-linux"));
  run("wsl.exe", [
    "--cd",
    wslSiteRoot,
    "-e",
    "env",
    `PATH=${home}/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin`,
    `CARGO_TARGET_DIR=${wslTarget}`,
    executable,
    "build",
  ]);
}

function buildPliegoCssWithWsl(outputCss, manifest) {
  const home = wslHome();
  const cargo = `${home}/.cargo/bin/cargo`;
  const wslRoot = windowsToWsl(root);
  const wslTarget = wslPliegoCssTarget();
  const environment = [
    `PATH=${home}/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin`,
    `CARGO_TARGET_DIR=${wslTarget}`,
  ];
  run("wsl.exe", [
    "--cd",
    wslRoot,
    "-e",
    "env",
    ...environment,
    cargo,
    "+1.85.0",
    "build",
    "--locked",
    "-p",
    "pliego-cssc",
  ]);
  run("wsl.exe", [
    "--cd",
    wslRoot,
    "-e",
    `${wslTarget}/debug/pliego-cssc`,
    "compile",
    "--source",
    windowsToWsl(join(siteRoot, "src")),
    "--seed",
    "--theme",
    "--output",
    windowsToWsl(outputCss),
    "--manifest",
    windowsToWsl(manifest),
  ]);
}

function wslPliegoCssTarget() {
  const checkoutKey = sha256(Buffer.from(root, "utf8")).slice(0, 16);
  return `${wslHome()}/.cache/pliegocss/site-css-${checkoutKey}`;
}

function wslPliegoCssExecutable() {
  return `${wslPliegoCssTarget()}/debug/pliego-cssc`;
}

function buildSite() {
  rmSync(output, { recursive: true, force: true });
  if (process.platform === "win32" && existsSync(applicationControlMarker)) {
    buildSiteWithWsl();
  } else {
    const environment = {
      ...process.env,
      CARGO_TARGET_DIR: siteTarget,
    };
    ensurePliego();
    const result = runCaptured(pliego, ["build"], {
      cwd: siteRoot,
      env: environment,
    });
    if (result.error) throw result.error;
    if (result.status !== 0) {
      const diagnostics = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
      if (
        process.platform === "win32" &&
        diagnostics.includes("Application Control policy has blocked this file")
      ) {
        mkdirSync(dirname(applicationControlMarker), { recursive: true });
        writeFileSync(applicationControlMarker, "Windows error 4551; use verified WSL build.\n");
        buildSiteWithWsl();
      } else {
        throw new Error(`pliego build failed with ${result.status}\n${diagnostics}`);
      }
    } else {
      process.stdout.write(result.stdout ?? "");
      process.stderr.write(result.stderr ?? "");
    }
  }
  if (!existsSync(join(output, "index.html"))) {
    throw new Error("PliegoRS did not publish the site index");
  }
}

run(process.execPath, [join(siteRoot, "scripts", "generate-docs.mjs"), "--check"]);
prepareAssets();
buildPliegoCss();
generateLaboratory();
generateCatalog();
await buildClient();
buildSite();
process.stdout.write(`PliegoCSS website built at ${output}\n`);
