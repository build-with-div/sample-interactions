// Builds the reference pages into out/reference/ (ignored): each page is one
// HTML file with its bundle inline, in React's production build.
//
//     cd reference && npm ci && node build.mjs

import { build } from "esbuild";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const output = join(here, "..", "out", "reference");
const PAGES = ["panels"];

mkdirSync(output, { recursive: true });
for (const page of PAGES) {
  const result = await build({
    entryPoints: [join(here, "src", `${page}.jsx`)],
    bundle: true,
    format: "iife",
    jsx: "automatic",
    define: { "process.env.NODE_ENV": '"production"' },
    write: false,
    logLevel: "warning",
  });
  const script = result.outputFiles[0].text.replaceAll("</script", "<\/script");
  const versions = ["react", "react-dom", "react-resizable-panels"]
    .map((name) => `${name} ${JSON.parse(readFileSync(join(here, "node_modules", name, "package.json"), "utf8")).version}`)
    .join(", ");
  const html = `<!doctype html>
<meta charset="utf-8">
<meta name="generator" content="${versions}">
<title>${page}</title>
<style>html, body { margin: 0; } #host { width: 800px; height: 400px; }</style>
<div id="host"></div>
<script>${script}</script>
`;
  writeFileSync(join(output, `${page}.html`), html);
  console.log(`${page}.html: ${versions}`);
}
