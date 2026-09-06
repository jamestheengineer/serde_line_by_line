// Runs every expansion case out of the built expander module and diffs it
// against the transcript CI asserts natively.
//
// This is the check that matters for N3: a `#[cfg(target_arch)]` anywhere in
// serde_derive, syn or prettyplease — or anything that formats differently on
// wasm32 — would make the site show one expansion while the tests assert
// another, and nothing else would notice.
//
// __REPO__ is substituted by xtask before this file is run.

import { readFileSync } from "node:fs";
import { initSync, expand, derive_version } from "__REPO__/app/static/wasm/expander.js";

initSync({ module: readFileSync("__REPO__/app/static/wasm/expander_bg.wasm") });

const cases = readFileSync("__REPO__/expand/expected.txt", "utf8");
const want = cases.trimEnd();

// Rebuild the transcript from the browser module, in expand::transcript()'s
// format: the sources come from the same cases/ directory, baked into both.
const names = [...cases.matchAll(/^### (\S+) (Serialize|Deserialize)$/gm)];
const seen = [];
for (const [, name] of names) {
  if (!seen.includes(name)) seen.push(name);
}
if (seen.length === 0) {
  console.error("no cases found in expand/expected.txt");
  process.exit(1);
}

let out = "";
for (const name of seen) {
  const source = readFileSync(`__REPO__/expand/cases/${name}.rs`, "utf8");
  for (const derive of ["Serialize", "Deserialize"]) {
    out += `### ${name} ${derive}\n`;
    out += expand(source, derive).trimEnd();
    out += "\n\n";
  }
}
const got = out.trimEnd();

if (got === want) {
  console.log(`  expander: ${seen.length} cases match, serde_derive ${derive_version()}`);
  process.exit(0);
}

const g = got.split("\n");
const w = want.split("\n");
for (let i = 0; i < Math.max(g.length, w.length); i++) {
  if (g[i] !== w[i]) {
    console.error("  expander: browser expansion differs from expected.txt");
    console.error(`    line ${i + 1}`);
    console.error(`    native : ${w[i] ?? "<end>"}`);
    console.error(`    browser: ${g[i] ?? "<end>"}`);
    break;
  }
}
process.exit(1);
