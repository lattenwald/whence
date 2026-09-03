import assert from "node:assert/strict";
import path from "node:path";
import { Engine, EngineError } from "../src/engine";
import { replayHost } from "../src/hostReplay";

const fixture = process.env.WHENCE_TEST_REPLAY!;
const bin = process.env.WHENCE_TEST_BIN!;

function spawn(): Engine {
  return Engine.spawn({ command: [bin, "serve"], cwd: fixture, host: replayHost(fixture), log: () => {} });
}

describe("engine", () => {
  it("initialises and traces through the replay host", async () => {
    const engine = spawn();
    const init = await engine.initialize(fixture);
    assert.ok(init.languages.includes("erlang"));
    const tree = await engine.trace({ file: path.join(fixture, "a.erl"), line: 6, col: 4 });
    assert.equal(tree.root.label, "Z");
    assert.ok(tree.root.children.length > 0);
    await engine.dispose();
  });

  it("rejects a second trace while one is running", async () => {
    const engine = spawn();
    await engine.initialize(fixture);
    const first = engine.trace({ file: path.join(fixture, "a.erl"), line: 6, col: 4 });
    await assert.rejects(engine.trace({ file: path.join(fixture, "a.erl"), line: 6, col: 4 }), (e: EngineError) => e.message === "busy");
    await first;
    await engine.dispose();
  });

  it("surfaces engine errors with their code", async () => {
    const engine = spawn();
    await engine.initialize(fixture);
    await assert.rejects(engine.trace({ file: path.join(fixture, "a.erl"), line: 0, col: 0 }), (e: EngineError) => e.code === -32002);
    await engine.dispose();
  });

  it("fails the pending trace when the process dies", async () => {
    let exited: number | null | undefined;
    const engine = Engine.spawn({
      command: [bin, "serve"],
      cwd: fixture,
      host: () => new Promise(() => {}), // never answers, so the trace is stuck inside the engine
      log: () => {},
      onExit: (code) => (exited = code),
    });
    await engine.initialize(fixture);
    const pending = engine.trace({ file: path.join(fixture, "a.erl"), line: 6, col: 4 });
    engine.kill();
    await assert.rejects(pending, (e: EngineError) => e.message.startsWith("engine exited"));
    await engine.exited;
    assert.notEqual(exited, undefined);
  });
});
