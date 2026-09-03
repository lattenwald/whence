import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { TARGETS } from "../src/targets";

const repo = path.resolve(__dirname, "..", "..", "..");

describe("targets", () => {
  it("maps exactly the triples the release workflow builds", () => {
    const yml = readFileSync(path.join(repo, ".github", "workflows", "release.yml"), "utf8");
    const released = new Set<string>();
    for (const m of yml.matchAll(/^\s*- target:\s*(\S+)/gm)) {
      released.add(m[1]!);
    }
    assert.deepEqual(new Set(Object.keys(TARGETS)), released);
  });
});
