import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import * as vscode from "vscode";
import type { WhenceApi } from "../src/extension";
import type { Tree } from "../src/types";

const fixture = process.env.WHENCE_TEST_REPLAY!;
const bin = process.env.WHENCE_TEST_BIN!;
const file = path.join(fixture, "a.erl");
const run = promisify(execFile);

async function api(): Promise<WhenceApi> {
  return vscode.extensions.getExtension<WhenceApi>("lattenwald.whence")!.activate();
}

describe("record", () => {
  after(async () => (await api()).stopEngines());

  it("writes a fixture that replays to the same tree", async () => {
    const { recordAt, tree } = await api();
    const out = mkdtempSync(path.join(os.tmpdir(), "whence-rec-"));
    const conflicts = await recordAt(out, file, 6, 4);
    assert.deepEqual(conflicts, []);
    const live = tree.current!.tree;

    const meta = JSON.parse(readFileSync(path.join(out, "whence-record.json"), "utf8"));
    assert.deepEqual([meta.root, meta.file, meta.line, meta.col], [fixture, "a.erl", 6, 4]);

    const { stdout } = await run(bin, ["replay", out, "a.erl:7:5", "--json"]);
    const replayed = JSON.parse(stdout) as Tree;
    // `loc.file` is absolute, so the two traces differ only by their roots.
    const rooted = (node: Tree["root"], root: string): unknown => JSON.parse(JSON.stringify(node).split(`${root}/`).join(""));
    assert.deepEqual(rooted(replayed.root, out), rooted(live.root, fixture));
  });

  it("refuses a non-empty directory and a second concurrent recording", async () => {
    const { recordAt } = await api();
    const used = mkdtempSync(path.join(os.tmpdir(), "whence-rec-"));
    writeFileSync(path.join(used, "host.json"), "{}");
    await assert.rejects(recordAt(used, file, 6, 4), /not empty/);

    const a = mkdtempSync(path.join(os.tmpdir(), "whence-rec-"));
    const b = mkdtempSync(path.join(os.tmpdir(), "whence-rec-"));
    const first = recordAt(a, file, 6, 4);
    await assert.rejects(recordAt(b, file, 6, 4), (e: Error) => e.message.includes(a));
    await first;
    // The refused recording must not have taken the first one's fixture or host slot with it.
    assert.deepEqual(readdirSync(b), []);
    assert.ok(JSON.parse(readFileSync(path.join(a, "host.json"), "utf8")).definition);
    assert.deepEqual(await recordAt(mkdtempSync(path.join(os.tmpdir(), "whence-rec-")), file, 6, 4), []);
  });
});
