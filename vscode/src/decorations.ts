import * as vscode from "vscode";
import type { Node, Tree } from "./types";

function rangeOf(doc: vscode.TextDocument, node: Node): vscode.Range {
  const pos = new vscode.Position(node.loc.line, node.loc.col);
  return doc.getWordRangeAtPosition(pos) ?? new vscode.Range(pos, pos.translate(0, node.label.length));
}

export class Decorations implements vscode.Disposable {
  private readonly all = vscode.window.createTextEditorDecorationType({
    backgroundColor: new vscode.ThemeColor("editor.wordHighlightBackground"),
    overviewRulerColor: new vscode.ThemeColor("editor.wordHighlightBackground"),
    overviewRulerLane: vscode.OverviewRulerLane.Center,
  });
  private readonly strong = vscode.window.createTextEditorDecorationType({
    backgroundColor: new vscode.ThemeColor("editor.wordHighlightStrongBackground"),
    border: "1px solid",
    borderColor: new vscode.ThemeColor("editor.wordHighlightStrongBorder"),
  });
  private nodes: Node[] = [];
  private selected: Node | undefined;
  private readonly sub = vscode.window.onDidChangeVisibleTextEditors(() => this.apply());

  set(tree: Tree | null): void {
    this.nodes = [];
    this.selected = undefined;
    if (tree) {
      const walk = (n: Node) => (this.nodes.push(n), n.children.forEach(walk));
      walk(tree.root);
    }
    this.apply();
  }

  select(node: Node | undefined): void {
    this.selected = node;
    this.apply();
  }

  private apply(): void {
    for (const editor of vscode.window.visibleTextEditors) {
      const file = editor.document.uri.fsPath;
      const here = this.nodes.filter((n) => n.loc.file === file);
      editor.setDecorations(this.all, here.map((n) => rangeOf(editor.document, n)));
      editor.setDecorations(this.strong, this.selected && this.selected.loc.file === file ? [rangeOf(editor.document, this.selected)] : []);
    }
  }

  dispose(): void {
    this.sub.dispose();
    this.all.dispose();
    this.strong.dispose();
  }
}
