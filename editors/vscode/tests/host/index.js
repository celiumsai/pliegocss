"use strict";

const assert = require("node:assert/strict");
const path = require("node:path");
const vscode = require("vscode");

/**
 * @template T
 * @param {() => T | undefined | Promise<T | undefined>} probe
 * @param {string} label
 * @param {number} [timeoutMs]
 * @returns {Promise<T>}
 */
async function waitFor(probe, label, timeoutMs = 15_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = await probe();
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`timed out waiting for ${label}`);
}

async function run() {
  const root = process.env.PLIEGOCSS_HOST_WORKSPACE;
  assert.ok(root, "missing host workspace path");
  const extension = vscode.extensions.getExtension("celiums.pliegocss-vscode");
  assert.ok(extension, "development extension is not installed in the host");
  await extension.activate();

  const sourcePath = path.join(root, "src", "view.rs");
  const sourceUri = vscode.Uri.file(sourcePath);
  const document = await vscode.workspace.openTextDocument(sourceUri);
  await vscode.window.showTextDocument(document);
  assert.equal(document.languageId, "rust");

  const source = document.getText();
  const gap = source.indexOf("gap-4") + 2;
  const definitions = await waitFor(
    async () => {
      const result = await vscode.commands.executeCommand(
        "vscode.executeDefinitionProvider",
        sourceUri,
        document.positionAt(gap),
      );
      return Array.isArray(result) && result.length > 0 ? result : undefined;
    },
    "Project Index definition",
  );
  const target = definitions[0].targetUri ?? definitions[0].uri;
  const range = definitions[0].targetSelectionRange ?? definitions[0].range;
  const normalizedTarget = path.normalize(target.fsPath);
  const normalizedExpected = path.normalize(path.join(root, "out", "app.css"));
  assert.equal(
    process.platform === "win32" ? normalizedTarget.toLowerCase() : normalizedTarget,
    process.platform === "win32" ? normalizedExpected.toLowerCase() : normalizedExpected,
  );
  assert.equal(range.start.line, 0);
  assert.equal(range.start.character, 4);
  assert.equal(range.end.character, 16);

  const invalid = 'fn view(){let _=pc!("flex unknown-thing");}';
  const edit = new vscode.WorkspaceEdit();
  edit.replace(sourceUri, new vscode.Range(document.positionAt(0), document.positionAt(source.length)), invalid);
  assert.equal(await vscode.workspace.applyEdit(edit), true);
  const diagnostic = await waitFor(
    () => {
      const values = vscode.languages.getDiagnostics(sourceUri);
      return values.find((value) => value.code === "PCS001");
    },
    "compiler semantic diagnostic",
  );
  assert.equal(diagnostic.message, "unknown utility `unknown-thing`");
  assert.equal(diagnostic.range.start.character, invalid.indexOf("unknown-thing"));
  assert.equal(
    diagnostic.range.end.character,
    invalid.indexOf("unknown-thing") + "unknown-thing".length,
  );
}

module.exports = { run };
