// Build the engine for the plugin: cargo (wasm32, release) -> wasm-bindgen
// (--target web for the worker, --target nodejs for the test harness) ->
// wasm-opt -Oz. Outputs land in src/generated/ (gitignored).
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../..");
const out = resolve(here, "../src/generated");
const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { stdio: "inherit", cwd: root, ...opts });

// Repo-local cargo home (see scripts/env.sh); a worktree falls back to the main checkout's.
const cargoHome =
  process.env.ONTOMARK_CARGO_HOME ??
  [resolve(root, ".cargo-home"), resolve(root, "../ontomark/.cargo-home")].find((p) => existsSync(p)) ??
  process.env.CARGO_HOME;
const env = { ...process.env, CARGO_HOME: cargoHome };
// Older distro optimisers can cap wasm-bindgen's growable externref table.
// Keep the optimiser used locally and in CI reproducible.
const optimizer = execFileSync("wasm-opt", ["--version"], { encoding: "utf8" }).trim();
if (!/^wasm-opt version 132(?: \(version_132\))?$/.test(optimizer)) {
  throw new Error(`Binaryen 132 required, found ${optimizer}`);
}
run("cargo", ["build", "-p", "ontomark-wasm", "--target", "wasm32-unknown-unknown", "--release"], { env });
const target = resolve(root, process.env.CARGO_TARGET_DIR ?? "target");
const wasm = join(target, "wasm32-unknown-unknown/release/ontomark_wasm.wasm");
mkdirSync(join(out, "web"), { recursive: true });
mkdirSync(join(out, "node"), { recursive: true });
run("wasm-bindgen", [wasm, "--target", "web", "--out-dir", join(out, "web")]);
run("wasm-bindgen", [wasm, "--target", "nodejs", "--out-dir", join(out, "node")]);
for (const dir of ["web", "node"]) {
  const file = join(out, dir, "ontomark_wasm_bg.wasm");
  const before = statSync(file).size;
  run("wasm-opt", ["-Oz", "--enable-bulk-memory", "--enable-nontrapping-float-to-int", "--enable-sign-ext", "--enable-mutable-globals", "--strip-debug", file, "-o", file]);
  console.log(`wasm-opt ${dir}: ${before} -> ${statSync(file).size} bytes`);
}
copyFileSync(join(out, "web", "ontomark_wasm_bg.wasm"), join(out, "ontomark.wasm"));
// The nodejs-target glue is CommonJS; the plugin package is ESM.
writeFileSync(join(out, "node", "package.json"), '{ "type": "commonjs" }\n');
