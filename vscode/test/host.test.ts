import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import * as vscode from "vscode";
import { definition, dispatch, references, text } from "../src/host";

const dir = mkdtempSync(path.join(os.tmpdir(), "whence-host-"));
const file = path.join(dir, "x.whencetest");
const other = path.join(dir, "y.whencetest");
writeFileSync(file, "alpha beta\ngamma\n");
writeFileSync(other, "delta\n");

const selector: vscode.DocumentSelector = { pattern: "**/*.whencetest" };
const subs: vscode.Disposable[] = [];

describe("host", () => {
  after(() => subs.forEach((s) => s.dispose()));

  it("serves unsaved edits for an open document and disk content otherwise", async () => {
    assert.equal((await text({ file: other })).text, "delta\n");
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
    const edit = new vscode.WorkspaceEdit();
    edit.insert(doc.uri, new vscode.Position(0, 0), "EDIT ");
    await vscode.workspace.applyEdit(edit);
    assert.equal((await text({ file })).text, "EDIT alpha beta\ngamma\n");
  });

  it("flattens Locations and LocationLinks from real providers and drops duplicates", async () => {
    const uri = vscode.Uri.file(other);
    subs.push(
      vscode.languages.registerDefinitionProvider(selector, {
        provideDefinition: () => [
          new vscode.Location(uri, new vscode.Range(0, 0, 0, 5)),
          { targetUri: uri, targetRange: new vscode.Range(0, 0, 1, 0), targetSelectionRange: new vscode.Range(0, 0, 0, 5) },
          { targetUri: uri, targetRange: new vscode.Range(0, 0, 1, 0), targetSelectionRange: new vscode.Range(0, 2, 0, 3) },
        ] as unknown as vscode.LocationLink[],
      }),
    );
    const out = await definition({ file, line: 0, col: 1 });
    assert.deepEqual(out, [
      { file: other, range: { start: { line: 0, col: 0 }, end: { line: 0, col: 5 } } },
      { file: other, range: { start: { line: 0, col: 2 }, end: { line: 0, col: 3 } } },
    ]);
  });

  it("answers an empty list when providers find nothing", async () => {
    subs.push(vscode.languages.registerReferenceProvider(selector, { provideReferences: () => [] }));
    assert.deepEqual(await references({ file, line: 0, col: 1, includeDeclaration: false }), []);
  });

  it("turns an unreadable file into an error, not an empty answer", async () => {
    await assert.rejects(dispatch("host/definition", { file: path.join(dir, "missing.whencetest"), line: 0, col: 0 }));
  });

  it("rejects an unknown method with the JSON-RPC code", async () => {
    await assert.rejects(dispatch("host/nope", {}), (e: { code: number }) => e.code === -32601);
  });
});
