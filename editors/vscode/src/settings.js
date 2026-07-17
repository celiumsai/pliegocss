"use strict";

const THEME_MODES = new Set(["discover", "seed", "config"]);

/**
 * @typedef {{serverPath:string, compilerPath:string, themeMode:string, themeConfig:string}} Settings
 */

/**
 * Builds the exact native-server invocation without consulting the filesystem.
 * @param {Settings} settings
 * @returns {{command:string,args:string[]}}
 */
function serverInvocation(settings) {
  const serverPath = settings.serverPath.trim();
  const compilerPath = settings.compilerPath.trim();
  if (!serverPath) throw new Error("pliegocss.server.path cannot be empty");
  if (!compilerPath) throw new Error("pliegocss.compiler.path cannot be empty");
  if (!THEME_MODES.has(settings.themeMode)) {
    throw new Error(`unsupported PliegoCSS theme mode: ${settings.themeMode}`);
  }
  const args = ["--pliego-cssc", compilerPath];
  if (settings.themeMode === "seed") args.push("--seed");
  if (settings.themeMode === "config") {
    const config = settings.themeConfig.trim();
    if (!config) throw new Error("pliegocss.theme.config cannot be empty in config mode");
    args.push("--config", config);
  }
  return { command: serverPath, args };
}

module.exports = { serverInvocation };
