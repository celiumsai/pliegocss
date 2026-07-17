import { build } from "esbuild";

await build({
  entryPoints: ["src/extension.js"],
  outfile: "dist/extension.js",
  bundle: true,
  platform: "node",
  format: "cjs",
  target: "node22",
  external: ["vscode"],
  sourcemap: false,
  minify: true,
  legalComments: "none",
  logLevel: "info",
});
