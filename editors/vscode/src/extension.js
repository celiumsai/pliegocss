"use strict";

const vscode = require("vscode");
const { LanguageClient } = require("vscode-languageclient/node");
const { serverInvocation } = require("./settings");

/** @type {LanguageClient | undefined} */
let client;
let transition = Promise.resolve();

function readSettings() {
  const configuration = vscode.workspace.getConfiguration("pliegocss");
  return {
    serverPath: configuration.get("server.path", "pliego-css-lsp"),
    compilerPath: configuration.get("compiler.path", "pliego-cssc"),
    projectIndexPath: configuration.get("projectIndex.path", ""),
    themeMode: configuration.get("theme.mode", "discover"),
    themeConfig: configuration.get("theme.config", "pliego.theme.toml"),
  };
}

async function stop() {
  const active = client;
  client = undefined;
  if (active) await active.stop();
}

async function start() {
  await stop();
  const invocation = serverInvocation(readSettings());
  const folder = vscode.workspace.workspaceFolders?.[0];
  const executable = {
    command: invocation.command,
    args: invocation.args,
    options: folder ? { cwd: folder.uri.fsPath } : undefined,
  };
  client = new LanguageClient(
    "pliegocss",
    "PliegoCSS",
    { run: executable, debug: executable },
    {
      documentSelector: [{ scheme: "file", language: "rust" }],
      synchronize: { configurationSection: "pliegocss" },
    },
  );
  await client.start();
}

function restart() {
  transition = transition.then(start, start).catch((error) => {
    void vscode.window.showErrorMessage(`PliegoCSS language server: ${String(error)}`);
  });
  return transition;
}

/** @param {vscode.ExtensionContext} context */
async function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand("pliegocss.restartServer", restart),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("pliegocss")) void restart();
    }),
  );
  await restart();
}

async function deactivate() {
  await transition;
  await stop();
}

module.exports = { activate, deactivate };
