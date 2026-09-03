import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const repo = path.resolve(__dirname, "..", "..", "..");

describe("version", () => {
  it("matches the engine crate", () => {
    const cargo = readFileSync(path.join(repo, "engine", "Cargo.toml"), "utf8");
    const crate = /\[package\][^[]*?\nversion\s*=\s*"([^"]+)"/.exec(cargo)?.[1];
    const pkg = JSON.parse(readFileSync(path.join(repo, "vscode", "package.json"), "utf8")) as {
      version: string;
    };
    assert.equal(pkg.version, crate);
  });
});
