// Runs every example from the built playground module and diffs its output
// against the transcript CI asserts natively.
//
// Written for node, not a browser, so it can run as a build gate. The module
// is a wasm-bindgen `--target web` build, which loads by URL in a browser and
// by `initSync` with the bytes here; the compiled code either way is the same
// module the reader runs.
//
// __REPO__ is substituted by xtask before this file is run.

import { readFileSync } from "node:fs";
import { initSync, run, examples } from "__REPO__/app/static/wasm/playground.js";

initSync({ module: readFileSync("__REPO__/app/static/wasm/playground_bg.wasm") });

const names = examples().split("\n").filter(Boolean);
if (names.length === 0) {
  console.error("the playground contains no examples");
  process.exit(1);
}

let bad = 0;
for (const name of names) {
  const got = run(name).trimEnd();
  const want = readFileSync(`__REPO__/examples/${name}/expected.txt`, "utf8").trimEnd();
  if (got === want) continue;
  bad++;
  console.error(`  ${name}: browser output differs from expected.txt`);
  const g = got.split("\n");
  const w = want.split("\n");
  for (let i = 0; i < Math.max(g.length, w.length); i++) {
    if (g[i] !== w[i]) {
      console.error(`    line ${i + 1}\n      wasm:     ${JSON.stringify(g[i])}\n      expected: ${JSON.stringify(w[i])}`);
      break;
    }
  }
}

if (bad > 0) {
  console.error(`${bad} example(s) differ under wasm`);
  process.exit(1);
}
console.log(`  ${names.length} examples match expected.txt under wasm`);
