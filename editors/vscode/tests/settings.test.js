"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const { serverInvocation } = require("../src/settings");

const defaults = {
  serverPath: "pliego-css-lsp",
  projectIndexPath: "",
  themeMode: "discover",
  themeConfig: "pliego.theme.toml",
};

test("discovery starts only the in-process language server", () => {
  assert.deepEqual(serverInvocation(defaults), {
    command: "pliego-css-lsp",
    args: [],
  });
});

test("seed and config are exact mutually exclusive invocations", () => {
  assert.deepEqual(serverInvocation({ ...defaults, themeMode: "seed" }).args, [
    "--seed",
  ]);
  assert.deepEqual(
    serverInvocation({ ...defaults, themeMode: "config", themeConfig: "themes/app.toml" }).args,
    ["--config", "themes/app.toml"],
  );
});

test("Project Index navigation is explicit and path preserving", () => {
  assert.deepEqual(
    serverInvocation({ ...defaults, projectIndexPath: "target/site/assets/pliego.index.json" }).args,
    [
      "--project-index",
      "target/site/assets/pliego.index.json",
    ],
  );
});

test("empty server, invalid modes, and empty config fail closed", () => {
  assert.throws(() => serverInvocation({ ...defaults, serverPath: " " }), /server\.path/);
  assert.throws(() => serverInvocation({ ...defaults, themeMode: "automatic" }), /theme mode/);
  assert.throws(
    () => serverInvocation({ ...defaults, themeMode: "config", themeConfig: " " }),
    /theme\.config/,
  );
});
