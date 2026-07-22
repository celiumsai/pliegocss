import { spawnSync } from "node:child_process";
import { gzipSync } from "node:zlib";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const runtime = join(root, "benchmarks", "wasm-overhead", "runtime");
const results = join(root, "benchmarks", "results", "wasm-overhead.json");
const style = "flex items-center gap-4 rounded-lg bg-surface p-6";

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
    ...options,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed\n${result.stderr}`);
  }
  return result;
}

function createCrate(name, styled, identity) {
  const directory = join(runtime, name);
  mkdirSync(join(directory, "src"), { recursive: true });
  const dependency = styled
    ? `\n[dependencies]\npliego-css = { path = ${JSON.stringify(join(root, "crates", "pliego-css").replaceAll("\\", "/"))} }\n`
    : "";
  writeFileSync(
    join(directory, "Cargo.toml"),
    `[package]\nname = "check-${name}"\nversion = "0.0.0"\nedition = "2024"\n\n[lib]\ncrate-type = ["cdylib"]\n\n[workspace]\n${dependency}`,
  );
  const body = styled
    ? `use pliego_css::{Style, pc};\n\nconst STYLE: Style = pc!(${JSON.stringify(style)});\n\n#[unsafe(no_mangle)]\npub extern "C" fn style_id_low() -> u64 {\n    STYLE.id().get() as u64\n}\n`
    : `const STYLE_ID: u128 = 0x${identity};\n\n#[unsafe(no_mangle)]\npub extern "C" fn style_id_low() -> u64 {\n    STYLE_ID as u64\n}\n`;
  writeFileSync(join(directory, "src", "lib.rs"), body);
  return directory;
}

function build(directory, name) {
  const target = join(runtime, "targets", name);
  run(
    "cargo",
    ["build", "--release", "--target", "wasm32-unknown-unknown", "--manifest-path", join(directory, "Cargo.toml")],
    {
      env: {
        ...process.env,
        CARGO_TARGET_DIR: target,
        RUSTFLAGS: "-C opt-level=z -C lto=fat -C embed-bitcode=yes -C codegen-units=1 -C panic=abort",
      },
    },
  );
  return join(target, "wasm32-unknown-unknown", "release", `check_${name}.wasm`);
}

rmSync(runtime, { recursive: true, force: true });
mkdirSync(runtime, { recursive: true });
mkdirSync(dirname(results), { recursive: true });

const manifestPath = join(runtime, "style-manifest.json");
const cssPath = join(runtime, "style.css");
run("cargo", [
  "run",
  "--quiet",
  "-p",
  "pliego-cssc",
  "--",
  "compile",
  "--style",
  style,
  "--output",
  cssPath,
  "--manifest",
  manifestPath,
]);
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
if (
  manifest.schemaVersion !== 3 ||
  manifest.styleIdFormatVersion !== 2 ||
  manifest.classNameFormatVersion !== 1 ||
  manifest.themeIdFormatVersion !== 3
) {
  throw new Error(
    `expected manifest schema/identity formats 3/2/1/2, received ${manifest.schemaVersion}/${manifest.styleIdFormatVersion}/${manifest.classNameFormatVersion}/${manifest.themeIdFormatVersion}`,
  );
}
const identity = manifest.styles[0].styleId;

const plainWasm = readFileSync(build(createCrate("control", false, identity), "control"));
const styledWasm = readFileSync(build(createCrate("styledx", true, identity), "styledx"));
const report = {
  schemaVersion: 1,
  generatedAtUtc: new Date().toISOString(),
  target: "wasm32-unknown-unknown",
  rustflags: "-C opt-level=z -C lto=fat -C embed-bitcode=yes -C codegen-units=1 -C panic=abort",
  style,
  plain: { rawBytes: plainWasm.byteLength, gzipBytes: gzipSync(plainWasm, { level: 9 }).byteLength },
  styled: { rawBytes: styledWasm.byteLength, gzipBytes: gzipSync(styledWasm, { level: 9 }).byteLength },
};
report.delta = {
  rawBytes: report.styled.rawBytes - report.plain.rawBytes,
  gzipBytes: report.styled.gzipBytes - report.plain.gzipBytes,
};

writeFileSync(results, `${JSON.stringify(report, null, 2)}\n`);
process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
