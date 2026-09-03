import { existsSync } from "node:fs";
import path from "node:path";
import * as vscode from "vscode";
import { Decorations } from "./decorations";
import { Engine, EngineError } from "./engine";
import { host } from "./host";
import { replayHost } from "./hostReplay";
import { locationOf, WhenceTree, type Item } from "./tree";
import type { Tree } from "./types";

export type WhenceApi = {
  traceAt(file: string, line: number, col: number): Promise<Tree>;
  tree: WhenceTree;
  stopEngines(): Promise<void>;
};

function engineCommand(context: vscode.ExtensionContext): string[] {
  const test = context.extensionMode === vscode.ExtensionMode.Test ? process.env.WHENCE_TEST_BIN : undefined;
  const bin = test ?? path.join(context.extensionPath, "bin", process.platform === "win32" ? "whence.exe" : "whence");
  if (!existsSync(bin)) {
    throw new Error(`engine binary not found at ${bin}; this VSIX was packaged without it`);
  }
  return [bin, "serve"];
}

function rootOf(file: string): string {
  const folder = vscode.workspace.getWorkspaceFolder(vscode.Uri.file(file));
  if (!folder) {
    throw new Error(`open a folder containing ${path.basename(file)} to trace in it`);
  }
  return folder.uri.fsPath;
}

export function activate(context: vscode.ExtensionContext): WhenceApi {
  const log = vscode.window.createOutputChannel("Whence", { log: true });
  const tree = new WhenceTree();
  const decorations = new Decorations();
  const engines = new Map<string, Engine>();
  let last: { file: string; line: number; col: number } | null = null;
  let tracing = false;

  if (context.extensionMode === vscode.ExtensionMode.Test && process.env.WHENCE_TEST_REPLAY) {
    host.handle = replayHost(process.env.WHENCE_TEST_REPLAY);
  }

  async function engineFor(root: string): Promise<Engine> {
    const existing = engines.get(root);
    if (existing) {
      return existing;
    }
    const engine = Engine.spawn({
      command: engineCommand(context),
      cwd: root,
      // Resolved per request so a recorder installed later still sees the answers.
      host: (method, params) => host.handle(method, params),
      log: (line) => log.info(line),
      onExit: (code) => {
        engines.delete(root);
        if (code !== 0) {
          void vscode.window.showErrorMessage(`Whence: engine exited with ${code} (see the Whence output channel)`);
        }
      },
    });
    engines.set(root, engine);
    const init = await engine.initialize(root);
    log.info(`engine ${init.version}, languages: ${init.languages.join(", ")}`);
    return engine;
  }

  async function traceAt(file: string, line: number, col: number): Promise<Tree> {
    if (tracing) {
      throw new Error("a trace is already running");
    }
    tracing = true;
    try {
      const root = rootOf(file);
      const engine = await engineFor(root);
      const result = await vscode.window.withProgress(
        { location: vscode.ProgressLocation.Notification, title: "Whence: tracing…" },
        () => engine.trace({ file, line, col }),
      );
      last = { file, line, col };
      await tree.show(result, root);
      decorations.set(result);
      return result;
    } finally {
      tracing = false;
    }
  }

  function report(e: unknown): void {
    const message = e instanceof EngineError ? `${e.message} (${e.code})` : e instanceof Error ? e.message : String(e);
    log.error(message);
    void vscode.window.showErrorMessage(`Whence: ${message}`);
  }

  function fromEditor(): { file: string; line: number; col: number } | null {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.uri.scheme !== "file") {
      void vscode.window.showErrorMessage("Whence: the active editor has no file");
      return null;
    }
    const pos = editor.selection.start;
    return { file: editor.document.uri.fsPath, line: pos.line, col: pos.character };
  }

  async function reveal(item: Item | undefined, open: boolean): Promise<void> {
    const selected = tree.view.selection[0];
    const node = item?.kind === "node" ? item.node : selected?.kind === "node" ? selected.node : undefined;
    if (!node) {
      return;
    }
    const loc = locationOf(node);
    await vscode.window.showTextDocument(loc.uri, {
      selection: loc.range,
      preview: !open,
      preserveFocus: !open,
      viewColumn: vscode.ViewColumn.Active,
    });
  }

  async function stopEngines(): Promise<void> {
    await Promise.all([...engines.values()].map((e) => e.dispose()));
    engines.clear();
  }

  context.subscriptions.push(
    log,
    tree,
    decorations,
    { dispose: () => void stopEngines() },
    tree.onDidSelect((node) => decorations.select(node)),
    vscode.commands.registerCommand("whence.trace", async () => {
      const at = fromEditor();
      if (at) {
        await traceAt(at.file, at.line, at.col).catch(report);
      }
    }),
    vscode.commands.registerCommand("whence.rerun", async () => {
      if (last) {
        await traceAt(last.file, last.line, last.col).catch(report);
      }
    }),
    vscode.commands.registerCommand("whence.rerunFromNode", async (item: Item) => {
      if (item?.kind === "node") {
        await traceAt(item.node.loc.file, item.node.loc.line, item.node.loc.col).catch(report);
      }
    }),
    vscode.commands.registerCommand("whence.preview", (item: Item) => reveal(item, false)),
    vscode.commands.registerCommand("whence.open", (item?: Item) => reveal(item, true)),
    vscode.commands.registerCommand("whence.clear", () => {
      tree.clear();
      decorations.set(null);
    }),
  );

  return { traceAt, tree, stopEngines };
}

export function deactivate(): void {}
