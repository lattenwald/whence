/** Rust release triple → VSIX platform target. Must equal the release workflow's build matrix. */
export const TARGETS: Readonly<Record<string, string>> = {
  "x86_64-unknown-linux-gnu": "linux-x64",
  "aarch64-unknown-linux-gnu": "linux-arm64",
  "x86_64-apple-darwin": "darwin-x64",
  "aarch64-apple-darwin": "darwin-arm64",
  "x86_64-pc-windows-msvc": "win32-x64",
};

export function vsixTarget(triple: string): string {
  const target = TARGETS[triple];
  if (!target) {
    throw new Error(`no VSIX target for ${triple}; known: ${Object.keys(TARGETS).join(", ")}`);
  }
  return target;
}
