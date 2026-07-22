"use strict";

const THEME_MODES = new Set(["discover", "seed", "config"]);

/**
 * @typedef {{serverPath:string, projectIndexPath:string, themeMode:string, themeConfig:string}} Settings
 */

/**
 * Builds the exact native-server invocation without consulting the filesystem.
 * @param {Settings} settings
 * @returns {{command:string,args:string[]}}
 */
function serverInvocation(settings) {
  const serverPath = settings.serverPath.trim();
  if (!serverPath) throw new Error("pliegocss.server.path cannot be empty");
  if (!THEME_MODES.has(settings.themeMode)) {
    throw new Error(`unsupported PliegoCSS theme mode: ${settings.themeMode}`);
  }
  const args = [];
  const projectIndexPath = settings.projectIndexPath.trim();
  if (projectIndexPath) args.push("--project-index", projectIndexPath);
  if (settings.themeMode === "seed") args.push("--seed");
  if (settings.themeMode === "config") {
    const config = settings.themeConfig.trim();
    if (!config) throw new Error("pliegocss.theme.config cannot be empty in config mode");
    args.push("--config", config);
  }
  return { command: serverPath, args };
}

module.exports = { serverInvocation };
