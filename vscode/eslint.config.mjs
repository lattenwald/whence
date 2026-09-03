import tseslint from "typescript-eslint";

export default tseslint.config({ ignores: [".vscode-test/", "out/", "dist/"] }, ...tseslint.configs.recommended, {
  rules: {
    "@typescript-eslint/no-explicit-any": "off",
  },
});
