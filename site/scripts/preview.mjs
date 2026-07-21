// SPDX-License-Identifier: Apache-2.0

import { createServer } from "node:http";
import { existsSync, readFileSync, statSync } from "node:fs";
import { extname, join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..", "target", "site");
const port = Number.parseInt(process.env.PORT ?? "4173", 10);

if (!existsSync(join(root, "index.html"))) {
  throw new Error("build the site first with `pnpm site:build`");
}

const types = new Map([
  [".css", "text/css; charset=utf-8"],
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".avif", "image/avif"],
  [".png", "image/png"],
  [".svg", "image/svg+xml"],
  [".webp", "image/webp"],
  [".woff2", "font/woff2"],
  [".xml", "application/xml; charset=utf-8"],
]);

const server = createServer((request, response) => {
  const url = new URL(request.url ?? "/", "http://127.0.0.1");
  const decoded = decodeURIComponent(url.pathname);
  if (decoded.includes("\0") || decoded.split("/").includes("..")) {
    response.writeHead(400).end("Bad request");
    return;
  }
  const relative = decoded.replace(/^\/+/u, "");
  let path = join(root, relative);
  if (decoded.endsWith("/")) path = join(path, "index.html");
  if (!existsSync(path) || statSync(path).isDirectory()) {
    path = join(root, "404.html");
    response.statusCode = 404;
  }
  response.setHeader("content-type", types.get(extname(path)) ?? "application/octet-stream");
  response.setHeader("cache-control", "no-store");
  response.end(readFileSync(path));
});

server.listen(port, "127.0.0.1", () => {
  process.stdout.write(`PliegoCSS preview: http://127.0.0.1:${port}\n`);
});
