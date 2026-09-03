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
  private byFile = new Map<string, Node[]>();
  private selected: Node | undefined;
  private readonly sub = vscode.window.onDidChangeVisibleTextEditors(() => this.apply());

  set(tree: Tree | null): void {
    this.byFile = new Map();
    this.selected = undefined;
    const walk = (n: Node): void => {
      const here = this.byFile.get(n.loc.file);
      if (here) {
        here.push(n);
      } else {
        this.byFile.set(n.loc.file, [n]);
      }
      n.children.forEach(walk);
    };
    if (tree) {
      walk(tree.root);
    }
    this.apply();
  }

  select(node: Node | undefined): void {
    const was = this.selected;
    this.selected = node;
    for (const editor of vscode.window.visibleTextEditors) {
      const file = editor.document.uri.fsPath;
      if (file === was?.loc.file || file === node?.loc.file) {
        editor.setDecorations(this.strong, node?.loc.file === file ? [rangeOf(editor.document, node)] : []);
      }
    }
  }

  private apply(): void {
    for (const editor of vscode.window.visibleTextEditors) {
      const here = this.byFile.get(editor.document.uri.fsPath) ?? [];
      editor.setDecorations(
        this.all,
        here.map((n) => rangeOf(editor.document, n)),
      );
      const selected = this.selected?.loc.file === editor.document.uri.fsPath ? [rangeOf(editor.document, this.selected)] : [];
      editor.setDecorations(this.strong, selected);
    }
  }

  dispose(): void {
    this.sub.dispose();
    this.all.dispose();
    this.strong.dispose();
  }
}
