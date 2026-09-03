import * as vscode from "vscode";

export function activate(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("whence.trace", () => {
      void vscode.window.showInformationMessage("Whence: not implemented yet");
    }),
  );
}

export function deactivate(): void {}
