import { defineConfig } from "@vscode/test-cli";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, "..");
const fixture = path.join(repo, "engine", "tests", "fixtures", "erlang", "local_chain");
const bin =
  process.env.WHENCE_TEST_BIN ??
  path.join(repo, "target", "debug", process.platform === "win32" ? "whence.exe" : "whence");

export default defineConfig({
  files: "out/test/**/*.test.js",
  workspaceFolder: fixture,
  env: { WHENCE_TEST_BIN: bin, WHENCE_TEST_REPLAY: fixture },
  mocha: { ui: "bdd", timeout: 30000 },
});
