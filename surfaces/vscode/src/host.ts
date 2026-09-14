import { randomBytes } from "node:crypto";
import * as vscode from "vscode";

export function registerChat(context: vscode.ExtensionContext): void {
  let panel: vscode.WebviewPanel | undefined;
  context.subscriptions.push(
    vscode.commands.registerCommand("arut.chat", () => {
      if (panel) {
        panel.reveal(vscode.ViewColumn.Beside);
        return;
      }
      panel = openChat(context);
      panel.onDidDispose(() => {
        panel = undefined;
      });
    }),
  );
}

function openChat(context: vscode.ExtensionContext): vscode.WebviewPanel {
  const panel = vscode.window.createWebviewPanel(
    "arut.chat",
    "Arut",
    vscode.ViewColumn.Beside,
    {
      enableScripts: true,
      localResourceRoots: [vscode.Uri.joinPath(context.extensionUri, "dist")],
      retainContextWhenHidden: true,
    },
  );
  panel.webview.html = markup(panel.webview, context);
  return panel;
}

function markup(webview: vscode.Webview, context: vscode.ExtensionContext): string {
  const asset = (file: string) => webview.asWebviewUri(vscode.Uri.joinPath(context.extensionUri, "dist", file));
  const nonce = randomBytes(16).toString("base64");
  return `<!doctype html>
<html lang="${vscode.env.language}">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${nonce}' 'wasm-unsafe-eval'; connect-src ${webview.cspSource} data:;">
  <link rel="stylesheet" href="${asset("webview.css").toString()}">
</head>
<body>
  <main id="app"></main>
  <script nonce="${nonce}" type="module" src="${asset("webview.js").toString()}"></script>
</body>
</html>`;
}
