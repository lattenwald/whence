import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { TARGETS } from "../src/targets";

const repo = path.resolve(__dirname, "..", "..", "..");

describe("targets", () => {
  it("maps exactly the triples the release workflow builds and packages", () => {
    const yml = readFileSync(path.join(repo, ".github", "workflows", "release.yml"), "utf8");
    const jobs = new Map<string, number>();
    for (const m of yml.matchAll(/^\s*- target:\s*(\S+)/gm)) {
      jobs.set(m[1]!, (jobs.get(m[1]!) ?? 0) + 1);
    }
    assert.deepEqual(new Set(jobs.keys()), new Set(Object.keys(TARGETS)));
    // Each triple is listed by both the build matrix and the vsix matrix.
    assert.deepEqual(
      [...jobs].filter(([, n]) => n !== 2),
      [],
    );
  });
});
