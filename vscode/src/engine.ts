import * as cp from "node:child_process";
import {
  createMessageConnection,
  ErrorCodes,
  ResponseError,
  StreamMessageReader,
  StreamMessageWriter,
  type MessageConnection,
} from "vscode-jsonrpc/node";
import { HostError, type HostHandler, type Limits, type Tree } from "./types";

export type EngineOptions = {
  command: string[];
  cwd: string;
  host: HostHandler;
  log: (line: string) => void;
  onExit?: (code: number | null) => void;
};

export type TraceParams = { file: string; line: number; col: number; limits?: Limits };

/** An error response from the engine (`busy`, `E_HOST`, `E_NOT_IDENTIFIER`, …) or a dead process. */
export class EngineError extends Error {
  constructor(
    public readonly code: number,
    message: string,
  ) {
    super(message);
  }
}

const E_HOST = -32000;

export class Engine {
  private inflight: { reject: (e: Error) => void } | null = null;
  readonly exited: Promise<number | null>;

  private constructor(
    private readonly child: cp.ChildProcess,
    private readonly connection: MessageConnection,
    opts: EngineOptions,
  ) {
    this.exited = new Promise((resolve) => {
      child.once("exit", (code) => {
        connection.dispose();
        this.inflight?.reject(new EngineError(E_HOST, `engine exited ${code}`));
        this.inflight = null;
        opts.onExit?.(code);
        resolve(code);
      });
    });
  }

  static spawn(opts: EngineOptions): Engine {
    const [bin, ...args] = opts.command;
    if (!bin) {
      throw new Error("engine command is empty");
    }
    const child = cp.spawn(bin, args, { cwd: opts.cwd, stdio: ["pipe", "pipe", "pipe"] });
    child.stderr?.on("data", (chunk: Buffer) => opts.log(chunk.toString().trimEnd()));
    child.on("error", (err) => opts.log(`spawn failed: ${err.message}`));

    const connection = createMessageConnection(new StreamMessageReader(child.stdout!), new StreamMessageWriter(child.stdin!));
    connection.onRequest(async (method: string, params: unknown) => {
      try {
        return await opts.host(method, params);
      } catch (e) {
        const code = e instanceof HostError ? e.code : ErrorCodes.InternalError;
        throw new ResponseError(code, e instanceof Error ? e.message : String(e));
      }
    });
    connection.onError(([err]) => opts.log(`rpc error: ${err.message}`));
    connection.listen();
    return new Engine(child, connection, opts);
  }

  initialize(root: string): Promise<{ version: string; languages: string[] }> {
    return this.request("initialize", { root, capabilities: { documentHighlight: true } });
  }

  /** Single-flight: a second trace while one runs is rejected with `busy`. */
  async trace(params: TraceParams): Promise<Tree> {
    if (this.inflight) {
      throw new EngineError(ErrorCodes.InvalidRequest, "busy");
    }
    return new Promise<Tree>((resolve, reject) => {
      this.inflight = { reject };
      this.request<Tree>("whence/trace", params).then(resolve, reject).finally(() => {
        this.inflight = null;
      });
    });
  }

  private async request<T>(method: string, params: unknown): Promise<T> {
    if (this.child.exitCode !== null) {
      throw new EngineError(E_HOST, `engine exited ${this.child.exitCode}`);
    }
    try {
      return (await this.connection.sendRequest(method, params)) as T;
    } catch (e) {
      if (e instanceof ResponseError) {
        throw new EngineError(e.code, e.message);
      }
      throw e;
    }
  }

  /** `shutdown`, `exit`, then SIGKILL after a grace period. */
  async dispose(): Promise<void> {
    if (this.child.exitCode !== null) {
      return;
    }
    try {
      await this.request("shutdown", {});
      this.connection.sendNotification("exit");
    } catch {
      // Already gone or mid-trace; the kill below covers it.
    }
    const timer = setTimeout(() => this.child.kill("SIGKILL"), 2000);
    await this.exited;
    clearTimeout(timer);
  }

  kill(): void {
    this.child.kill("SIGKILL");
  }
}
