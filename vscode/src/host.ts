import * as vscode from "vscode";
import { ErrorCodes } from "vscode-jsonrpc/node";
import { HostError, type Highlight, type HostHandler, type Loc, type Location } from "./types";


async function document(file: string): Promise<vscode.TextDocument> {
  // Loads the file without showing an editor; returns the live document if it is already open.
  return vscode.workspace.openTextDocument(vscode.Uri.file(file));
}

function toRange(r: vscode.Range): Location["range"] {
  return { start: { line: r.start.line, col: r.start.character }, end: { line: r.end.line, col: r.end.character } };
}

/** Flattens Location | LocationLink (by target selection range) and drops duplicates by (file, range). */
export function locationsFrom(items: readonly (vscode.Location | vscode.LocationLink)[]): Location[] {
  const seen = new Set<string>();
  const out: Location[] = [];
  for (const item of items) {
    const uri = "targetUri" in item ? item.targetUri : item.uri;
    const range = "targetUri" in item ? (item.targetSelectionRange ?? item.targetRange) : item.range;
    const loc = { file: uri.fsPath, range: toRange(range) };
    const key = `${loc.file}:${loc.range.start.line}:${loc.range.start.col}:${loc.range.end.line}:${loc.range.end.col}`;
    if (!seen.has(key)) {
      seen.add(key);
      out.push(loc);
    }
  }
  return out;
}

export async function text(params: { file: string }): Promise<{ text: string }> {
  return { text: (await document(params.file)).getText() };
}

async function locations(command: string, params: Loc): Promise<Location[]> {
  const doc = await document(params.file);
  const result = await vscode.commands.executeCommand<(vscode.Location | vscode.LocationLink)[] | vscode.Location | undefined>(
    command,
    doc.uri,
    new vscode.Position(params.line, params.col),
  );
  if (!result) {
    return [];
  }
  return locationsFrom(Array.isArray(result) ? result : [result]);
}

export function definition(params: Loc): Promise<Location[]> {
  return locations("vscode.executeDefinitionProvider", params);
}

/** VS Code always includes the declaration; the engine skips references on declarations itself. */
export function references(params: Loc & { includeDeclaration: boolean }): Promise<Location[]> {
  return locations("vscode.executeReferenceProvider", params);
}

const KIND: Record<vscode.DocumentHighlightKind, Highlight["kind"]> = {
  [vscode.DocumentHighlightKind.Text]: "text",
  [vscode.DocumentHighlightKind.Read]: "read",
  [vscode.DocumentHighlightKind.Write]: "write",
};

export async function documentHighlight(params: Loc): Promise<Highlight[]> {
  const doc = await document(params.file);
  const result = await vscode.commands.executeCommand<vscode.DocumentHighlight[] | undefined>(
    "vscode.executeDocumentHighlights",
    doc.uri,
    new vscode.Position(params.line, params.col),
  );
  return (result ?? []).map((h) => ({ range: toRange(h.range), kind: KIND[h.kind ?? vscode.DocumentHighlightKind.Text] }));
}

export async function dispatch(method: string, params: any): Promise<unknown> {
  switch (method) {
    case "host/text":
      return text(params);
    case "host/definition":
      return definition(params);
    case "host/references":
      return references(params);
    case "host/documentHighlight":
      return documentHighlight(params);
    default:
      throw new HostError(ErrorCodes.MethodNotFound, `unknown method ${method}`);
  }
}

/** The engine calls `host.handle`; the recorder wraps it and tests replace it. */
export const host: { handle: HostHandler } = { handle: dispatch };
