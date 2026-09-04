import { readFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { ErrorCodes } from "vscode-jsonrpc/node";
import { HostError, type HostHandler, type Location } from "./types";

export type Sections = {
  definition?: Record<string, Location[]>;
  references?: Record<string, Location[]>;
  documentHighlight?: Record<string, unknown>;
  implementation?: Record<string, Location[]>;
};

/** Which `host.json` section answers a method; the recorder writes the same layout. */
export const SECTION: Record<string, keyof Sections> = {
  "host/definition": "definition",
  "host/references": "references",
  "host/documentHighlight": "documentHighlight",
  "host/implementation": "implementation",
};

/** Root-relative path with `/` separators, or null when `file` is not under `root`. */
export function relPath(file: string, root: string): string | null {
  const rel = path.relative(root, file);
  if (rel === "" || rel.startsWith("..") || path.isAbsolute(rel)) {
    return null;
  }
  return rel.split(path.sep).join("/");
}

/** Root-relative, else `$HOME/…`, else the absolute path. Same scheme as nvim/lua/whence/util.lua. */
export function portablePath(file: string, root: string): string {
  const rel = relPath(file, root);
  if (rel !== null) {
    return rel;
  }
  const home = relPath(file, os.homedir());
  return home === null ? file : `$HOME/${home}`;
}

/** Same scheme as engine/src/host_replay.rs; the fixtures are shared between hosts. */
export function fixtureKey(root: string, method: string, params: { file: string; line: number; col: number; includeDeclaration?: boolean }): string {
  let key = `${relPath(params.file, root) ?? params.file}:${params.line}:${params.col}`;
  if (method === "host/references") {
    key += params.includeDeclaration ? "|decl" : "|nodecl";
  }
  return key;
}

function absolutise(dir: string, file: string): string {
  if (file.startsWith("$HOME/")) {
    return path.join(os.homedir(), file.slice("$HOME/".length));
  }
  return path.isAbsolute(file) ? file : path.join(dir, file);
}

export async function loadFixture(dir: string): Promise<Sections> {
  const sections = JSON.parse(await readFile(path.join(dir, "host.json"), "utf8")) as Sections;
  for (const section of [sections.definition, sections.references, sections.implementation]) {
    for (const locations of Object.values(section ?? {})) {
      for (const loc of locations) {
        loc.file = absolutise(dir, loc.file);
      }
    }
  }
  return sections;
}

/** Answers host requests from a recorded fixture; `host/text` comes from disk. */
export function replayHost(dir: string): HostHandler {
  const loaded = loadFixture(dir);
  loaded.catch(() => {}); // The awaiting request reports it; without this it is an unhandled rejection.
  return async (method, params) => {
    if (method === "host/text") {
      return { text: await readFile(params.file, "utf8") };
    }
    const section = SECTION[method];
    if (!section) {
      throw new HostError(ErrorCodes.MethodNotFound, `unknown method ${method}`);
    }
    const answer = (await loaded)[section]?.[fixtureKey(dir, method, params)];
    if (answer === undefined) {
      throw new HostError(ErrorCodes.InternalError, "unrecorded");
    }
    return answer;
  };
}
