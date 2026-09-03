import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const lock = path.resolve(__dirname, "..", "..", "package-lock.json");
const registry = "https://registry.npmjs.org/";

describe("lockfile", () => {
  it("resolves every package from the public registry", () => {
    const packages = (JSON.parse(readFileSync(lock, "utf8")) as { packages: Record<string, { resolved?: string }> }).packages;
    const foreign = Object.entries(packages)
      .filter(([, p]) => p.resolved && !p.resolved.startsWith(registry))
      .map(([name]) => name);
    assert.deepEqual(foreign, [], `must resolve from ${registry}`);
  });
});
