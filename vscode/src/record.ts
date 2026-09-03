import { mkdir, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { host } from "./host";
import { fixtureKey, portablePath, relPath } from "./hostReplay";
import type { HostHandler, Location } from "./types";

const SECTION: Record<string, "definition" | "references" | "documentHighlight"> = {
  "host/definition": "definition",
  "host/references": "references",
  "host/documentHighlight": "documentHighlight",
};

type Recording = {
  dir: string;
  root: string;
  target: { file: string; line: number; col: number };
  engineVersion: string;
  recorded: { definition: Record<string, unknown>; references: Record<string, unknown>; documentHighlight: Record<string, unknown> };
  conflicts: string[];
  original: HostHandler;
};

let active: Recording | null = null;

async function copySource(rec: Recording, file: string, text: string): Promise<void> {
  const rel = relPath(file, rec.root);
  if (rel === null) {
    return;
  }
  const out = path.join(rec.dir, rel);
  await mkdir(path.dirname(out), { recursive: true });
  await writeFile(out, text);
}

async function capture(rec: Recording, method: string, params: any, result: unknown): Promise<void> {
  if (method === "host/text") {
    await copySource(rec, params.file, (result as { text: string }).text);
    return;
  }
  const section = SECTION[method];
  if (!section) {
    return;
  }
  const key = fixtureKey(rec.root, method, params);
  const answer =
    section === "documentHighlight"
      ? structuredClone(result)
      : (structuredClone(result) as Location[]).map((l) => ({ ...l, file: portablePath(l.file, rec.root) }));
  const first = rec.recorded[section][key];
  if (first !== undefined) {
    const label = `${method} ${key}`;
    if (JSON.stringify(first) !== JSON.stringify(answer) && !rec.conflicts.includes(label)) {
      rec.conflicts.push(label);
    }
    return;
  }
  rec.recorded[section][key] = answer;
}

export async function begin(opts: {
  dir: string;
  root: string;
  target: { file: string; line: number; col: number };
  engineVersion: string;
}): Promise<void> {
  if (active) {
    throw new Error(`a recording into ${active.dir} is already active`);
  }
  const rec: Recording = {
    ...opts,
    recorded: { definition: {}, references: {}, documentHighlight: {} },
    conflicts: [],
    original: host.handle,
  };
  active = rec; // Claimed before the first await, or two overlapping calls both pass the check.
  try {
    await mkdir(opts.dir, { recursive: true });
    if ((await readdir(opts.dir)).length > 0) {
      throw new Error(`${opts.dir} is not empty; record into a fresh directory`);
    }
  } catch (e) {
    active = null;
    throw e;
  }
  host.handle = async (method, params) => {
    const result = await rec.original(method, params);
    await capture(rec, method, params, result);
    return result;
  };
  active = rec;
}

export async function finish(): Promise<string[]> {
  const rec = active;
  if (!rec) {
    return [];
  }
  active = null;
  host.handle = rec.original;
  await writeFile(path.join(rec.dir, "host.json"), JSON.stringify(rec.recorded));
  const meta = {
    root: rec.root,
    file: relPath(rec.target.file, rec.root) ?? rec.target.file,
    line: rec.target.line,
    col: rec.target.col,
    engine_version: rec.engineVersion,
    recorded_at: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
    conflicts: rec.conflicts,
  };
  await writeFile(path.join(rec.dir, "whence-record.json"), JSON.stringify(meta));
  return rec.conflicts;
}
