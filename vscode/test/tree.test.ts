import assert from "node:assert/strict";
import path from "node:path";
import * as vscode from "vscode";
import type { WhenceApi } from "../src/extension";
import type { Item } from "../src/tree";
import type { Node } from "../src/types";

const fixture = process.env.WHENCE_TEST_REPLAY!;
const file = path.join(fixture, "a.erl");

async function api(): Promise<WhenceApi> {
  const ext = vscode.extensions.getExtension<WhenceApi>("lattenwald.whence")!;
  return ext.activate();
}

function nodeItems(items: Item[]): Node[] {
  return items.flatMap((i) => (i.kind === "node" ? [i.node] : []));
}

describe("tree", () => {
  after(async () => (await api()).stopEngines());

  it("maps every node to an item whose command carries that node, and the truncation line to none", async () => {
    const { tree } = await api();
    await tree.show(
      {
        root: {
          id: "a", kind: "binding", label: "Z", loc: { file, line: 5, col: 4 }, via: "match", snippet: "Z.", stop: null, truncated: 2,
          children: [{ id: "c", kind: "stop", label: "X", loc: { file, line: 2, col: 2 }, via: null, snippet: "f(X) ->", stop: { reason: "entry_point", detail: "" }, truncated: 0, children: [] }],
        },
        stats: { nodes: 2, truncated: 2, host_requests: 0, ms: 0 },
      },
      fixture,
    );
    const [root] = tree.getChildren();
    const children = tree.getChildren(root);
    assert.equal(children.length, 2);
    const stopItem = tree.getTreeItem(children[0]!);
    assert.deepEqual((stopItem.command!.arguments![0] as Item), children[0]);
    assert.equal(tree.getTreeItem(children[1]!).command, undefined);
    assert.deepEqual(tree.getParent(children[1]!), root);
  });

  it("shows a live trace, previews without taking focus, and opens on demand", async () => {
    const { tree, traceAt } = await api();
    const result = await traceAt(file, 6, 4);
    assert.equal(tree.current?.tree, result);
    const [root] = tree.getChildren();
    const first = nodeItems(tree.getChildren(root))[0]!;

    await vscode.commands.executeCommand("whence.preview", { kind: "node", node: first, root: fixture });
    const editor = vscode.window.visibleTextEditors.find((e) => e.document.uri.fsPath === first.loc.file)!;
    assert.deepEqual([editor.selection.start.line, editor.selection.start.character], [first.loc.line, first.loc.col]);

    await vscode.commands.executeCommand("whence.open", { kind: "node", node: first, root: fixture });
    assert.equal(vscode.window.activeTextEditor?.document.uri.fsPath, first.loc.file);
  });

  it("re-runs from a node and reuses the engine", async () => {
    const { tree, traceAt } = await api();
    const before = await traceAt(file, 6, 4);
    // The fixture only answers host requests for the trace at 6:4, so re-run from a node placed there.
    const target: Node = { ...nodeItems(tree.getChildren())[0]!, loc: { file, line: 6, col: 4 } };
    await vscode.commands.executeCommand("whence.rerunFromNode", { kind: "node", node: target, root: fixture });
    assert.notEqual(tree.current?.tree, before);
    assert.equal(tree.current?.tree.root.label, target.label);
  });

  it("survives an engine death and respawns on the next trace", async () => {
    const { traceAt, stopEngines } = await api();
    await traceAt(file, 6, 4);
    await stopEngines();
    const again = await traceAt(file, 6, 4);
    assert.equal(again.root.label, "Z");
  });
});
