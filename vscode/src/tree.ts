import path from "node:path";
import * as vscode from "vscode";
import type { Node, Tree } from "./types";

export type Item = { kind: "node"; node: Node; root: string } | { kind: "more"; count: number; parent: Node };

const ICON: Record<Node["kind"], string> = {
  binding: "symbol-variable",
  branch: "git-branch",
  param: "symbol-parameter",
  call_result: "symbol-method",
  field: "symbol-field",
  stop: "circle-slash",
};

function stopColor(node: Node): vscode.ThemeColor {
  const soft = node.stop?.reason === "external" || node.stop?.reason === "entry_point" || node.stop?.reason === "literal";
  return new vscode.ThemeColor(soft ? "problemsWarningIcon.foreground" : "problemsErrorIcon.foreground");
}

export function locationOf(node: Node): vscode.Location {
  return new vscode.Location(vscode.Uri.file(node.loc.file), new vscode.Position(node.loc.line, node.loc.col));
}

export class WhenceTree implements vscode.TreeDataProvider<Item>, vscode.Disposable {
  private readonly changed = new vscode.EventEmitter<Item | undefined>();
  private readonly selected = new vscode.EventEmitter<Node | undefined>();
  private parents = new Map<Node, Node>();
  private result: { tree: Tree; root: string } | null = null;
  readonly view: vscode.TreeView<Item>;
  readonly onDidChangeTreeData = this.changed.event;
  readonly onDidSelect = this.selected.event;

  constructor() {
    this.view = vscode.window.createTreeView("whence.tree", { treeDataProvider: this, showCollapseAll: true });
    this.view.onDidChangeSelection((e) => {
      const first = e.selection[0];
      this.selected.fire(first?.kind === "node" ? first.node : undefined);
    });
  }

  get current(): { tree: Tree; root: string } | null {
    return this.result;
  }

  async show(tree: Tree, root: string): Promise<void> {
    this.parents = new Map();
    const walk = (n: Node) => n.children.forEach((c) => (this.parents.set(c, n), walk(c)));
    walk(tree.root);
    this.result = { tree, root };
    await vscode.commands.executeCommand("setContext", "whence.hasResult", true);
    this.changed.fire(undefined);
    await this.view.reveal({ kind: "node", node: tree.root, root }, { expand: true, focus: false, select: false });
  }

  clear(): void {
    this.result = null;
    this.parents = new Map();
    this.changed.fire(undefined);
    void vscode.commands.executeCommand("setContext", "whence.hasResult", false);
  }

  getChildren(item?: Item): Item[] {
    if (!this.result) {
      return [];
    }
    if (!item) {
      return [{ kind: "node", node: this.result.tree.root, root: this.result.root }];
    }
    if (item.kind === "more") {
      return [];
    }
    const items: Item[] = item.node.children.map((node) => ({ kind: "node", node, root: item.root }));
    if (item.node.truncated > 0) {
      items.push({ kind: "more", count: item.node.truncated, parent: item.node });
    }
    return items;
  }

  getParent(item: Item): Item | undefined {
    const root = this.result?.root ?? "";
    const parent = item.kind === "more" ? item.parent : this.parents.get(item.node);
    return parent ? { kind: "node", node: parent, root } : undefined;
  }

  getTreeItem(item: Item): vscode.TreeItem {
    if (item.kind === "more") {
      const more = new vscode.TreeItem(`… ${item.count} more`, vscode.TreeItemCollapsibleState.None);
      more.id = `${item.parent.id}:more`;
      more.iconPath = new vscode.ThemeIcon("ellipsis");
      more.contextValue = "whence.truncated";
      more.tooltip = "Dropped by the fan-out bound; re-run from the parent with a higher limit to see them.";
      return more;
    }
    const { node, root } = item;
    const expandable = node.children.length > 0 || node.truncated > 0;
    const ti = new vscode.TreeItem(node.label, expandable ? vscode.TreeItemCollapsibleState.Expanded : vscode.TreeItemCollapsibleState.None);
    ti.id = node.id;
    ti.description = node.via ? `${node.via} · ${node.snippet}` : node.snippet;
    ti.iconPath = node.kind === "stop" ? new vscode.ThemeIcon(ICON.stop, stopColor(node)) : new vscode.ThemeIcon(ICON[node.kind]);
    ti.contextValue = node.kind === "stop" ? "whence.stop" : "whence.node";
    ti.command = { command: "whence.preview", title: "Preview", arguments: [item] };
    const rel = path.relative(root, node.loc.file) || node.loc.file;
    const md = new vscode.MarkdownString(`**${node.kind}** \`${rel}:${node.loc.line + 1}:${node.loc.col + 1}\``);
    if (node.stop) {
      md.appendMarkdown(`\n\n${node.stop.reason}: ${node.stop.detail}`);
    }
    ti.tooltip = md;
    return ti;
  }

  dispose(): void {
    this.view.dispose();
    this.changed.dispose();
    this.selected.dispose();
  }
}
