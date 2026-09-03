export type Pos = { line: number; col: number };
export type Range = { start: Pos; end: Pos };
export type Location = { file: string; range: Range };
export type Highlight = { range: Range; kind: "text" | "read" | "write" };
export type Loc = { file: string; line: number; col: number };

export type StopReason = "external" | "entry_point" | "literal" | "unresolved" | "limit";
export type Stop = { reason: StopReason; detail: string };
export type NodeKind = "binding" | "branch" | "param" | "call_result" | "field" | "stop";
export type Via = "match" | "rebind" | "mutation" | "arg" | "return" | "field_set" | "field";

export type Node = {
  id: string;
  kind: NodeKind;
  label: string;
  loc: Loc;
  via: Via | null;
  snippet: string;
  stop: Stop | null;
  children: Node[];
  truncated: number;
};

export type Tree = {
  root: Node;
  stats: { nodes: number; truncated: number; host_requests: number; ms: number };
};

export type Limits = Partial<{ depth: number; fanout: number; nodes: number; time_ms: number; split: boolean }>;

export type HostHandler = (method: string, params: any) => Promise<unknown>;

/** An error a host answer raises; `code` is the JSON-RPC error code sent back to the engine. */
export class HostError extends Error {
  constructor(
    public readonly code: number,
    message: string,
  ) {
    super(message);
  }
}
