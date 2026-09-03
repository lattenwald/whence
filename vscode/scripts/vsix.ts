import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { vsixTarget } from "../src/targets";

const triple = process.argv[2];
if (!triple) {
  console.error("usage: vsix.js <rust-target-triple> [path-to-binary]");
  process.exit(2);
}
const target = vsixTarget(triple);
const ext = path.resolve(__dirname, "..", "..");
const repo = path.resolve(ext, "..");
const exe = triple.includes("windows") ? "whence.exe" : "whence";
const source = process.argv[3] ?? path.join(repo, "target", triple, "release", exe);
if (!existsSync(source)) {
  console.error(`engine binary not found at ${source}`);
  process.exit(1);
}

mkdirSync(path.join(ext, "bin"), { recursive: true });
copyFileSync(source, path.join(ext, "bin", exe));
copyFileSync(path.join(repo, "LICENSE"), path.join(ext, "LICENSE"));
const version = (JSON.parse(readFileSync(path.join(ext, "package.json"), "utf8")) as { version: string }).version;
const out = path.join(ext, `whence-${target}-${version}.vsix`);
execFileSync("npx", ["vsce", "package", "--target", target, "--out", out], { cwd: ext, stdio: "inherit" });
console.log(out);
