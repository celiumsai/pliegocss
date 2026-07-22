import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const read = (path) => readFileSync(resolve(root, path), "utf8");

const compiler = read("crates/pliego-css-compiler/src/engine.rs");
const compilerLib = read("crates/pliego-css-compiler/src/lib.rs");
const cli = read("crates/pliego-cssc/src/main.rs");
const scanner = read("crates/pliego-cssc/src/source_candidates.rs");
const macros = read("crates/pliego-css-macros/src/lib.rs");
const lsp = read("crates/pliego-css-lsp/src/lib.rs");

function requireText(source, fragment, label) {
  if (!source.includes(fragment)) throw new Error(`${label} is missing ${fragment}`);
}

function forbidText(source, fragment, label) {
  if (source.includes(fragment)) throw new Error(`${label} still contains ${fragment}`);
}

for (const contract of [
  "pub struct CompileRequest",
  "pub struct CompileResult",
  "pub struct AnalysisHost",
  "pub struct PhysicalRulePlanner",
  "pub struct PcxRequest",
  "pub fn compile(&mut self, request: &CompileRequest)",
]) {
  requireText(compiler, contract, "shared compiler engine");
}
for (const exported of [
  "AnalysisHost",
  "CompileRequest",
  "CompileResult",
  "PhysicalRulePlanner",
  "PcxRequest",
]) {
  requireText(compilerLib, exported, "engine export");
}

for (const [source, label] of [
  [cli, "CLI/watch adapter"],
  [scanner, "Rust source scanner"],
  [macros, "procedural macro"],
  [lsp, "LSP adapter"],
]) {
  requireText(source, "AnalysisHost", label);
}
requireText(cli, "CompileRequest", "CLI/watch adapter");
requireText(scanner, "PcxRequest", "Rust source scanner");
requireText(macros, "PcxRequest", "procedural macro");
requireText(lsp, "PcxRequest", "LSP adapter");
requireText(cli, "engine: AnalysisHost", "watch cache");

forbidText(cli, "CssFragmentCache", "CLI physical planner");
forbidText(scanner, "analyze_cross_clause_conflicts", "Rust source scanner");
forbidText(macros, "fn cartesian_indices", "procedural macro");
forbidText(macros, "analyze_cross_clause_conflicts", "procedural macro");
forbidText(lsp, "Command::", "LSP adapter");
forbidText(lsp, "std::process::{", "LSP adapter");
forbidText(lsp, "TemporarySource", "LSP adapter");
forbidText(lsp, "run_source_check", "LSP adapter");

process.stdout.write(
  `${JSON.stringify(
    {
      schemaVersion: 1,
      engineApi: "CompileRequest->CompileResult",
      sharedPcxFrontend: ["macros", "scanner", "lsp"],
      sharedAdapters: ["cli", "watch", "lsp"],
      physicalPlanner: "compiler-owned",
      lspCompilerProcesses: 0,
    },
    null,
    2,
  )}\n`,
);
