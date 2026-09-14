import { randomBytes } from "node:crypto";
import {
  following,
  strings,
  t,
  type Changes,
  type ChatHandle,
  type ComposerHandle,
  type ConversationsHandle,
  type ProductSessionHandle,
} from "@arut/bindings-typescript";
import {
  projectionHost,
  type HostMessage,
  type PortMessage,
  type Published,
  type Wire,
} from "@arut/bindings-typescript/bridge";
import { chatIntent, type ChatProjection } from "@arut/chat-ui/port";
import * as vscode from "vscode";

// The session stays in the extension host (ADR 0011); the webview is a view
// over it. What crosses is one whole projection per change, and named intents
// back -- no sentence and no handle (ADR 0016). The following and posting is
// the binding package's (`projectionHost`); what this file states is which
// projections there are and what each intent does.

export function registerChat(context: vscode.ExtensionContext, session: ProductSessionHandle): void {
  let panel: vscode.WebviewPanel | undefined;
  context.subscriptions.push(
    vscode.commands.registerCommand("arut.chat", () => {
      if (panel) {
        panel.reveal(vscode.ViewColumn.Beside);
        return;
      }
      panel = openChat(context, session);
      panel.onDidDispose(() => {
        panel = undefined;
      });
    }),
  );
}

function openChat(context: vscode.ExtensionContext, session: ProductSessionHandle): vscode.WebviewPanel {
  const panel = vscode.window.createWebviewPanel(
    "arut.chat",
    t(strings, "app-name"),
    vscode.ViewColumn.Beside,
    {
      enableScripts: true,
      localResourceRoots: [vscode.Uri.joinPath(context.extensionUri, "dist")],
      // The core keeps running host-side; retaining the webview's DOM avoids
      // re-mounting the view every time the panel is hidden.
      retainContextWhenHidden: true,
    },
  );

  const post = (message: HostMessage) => void panel.webview.postMessage(message);
  const list = session.conversations();
  let chat = session.chat();
  let composer = chat.composer();
  let stopFollowing = following(composer);
  let host = projectionHost(published(chat, composer, list), post);

  // A conversation the person opened is a different handle: the old projections
  // stop, and the same declaration is made again over the new one.
  const bind = (next: ChatHandle) => {
    host.dispose();
    stopFollowing();
    composer.dispose();
    chat.dispose();
    chat = next;
    composer = chat.composer();
    stopFollowing = following(composer);
    host = projectionHost(published(chat, composer, list), post);
    host.publish();
  };

  panel.webview.html = markup(panel.webview, context, host.snapshot());
  panel.webview.onDidReceiveMessage(
    (message: PortMessage) => {
      if (message.type === "ready") {
        host.publish();
        return;
      }
      const intent = chatIntent(message.name, message.payload);
      if (intent === null) return;
      switch (intent.name) {
        case "newChat":
          bind(session.newChat());
          return;
        case "select": {
          list.select(intent.id);
          const next = intent.id === null ? undefined : session.selectChat(intent.id);
          if (next) bind(next);
          return;
        }
        case "draft":
          void composer.replace(intent.text);
          return;
        case "send":
          void chat.send(intent.text);
          return;
        case "query":
          list.setQuery(intent.text);
          return;
      }
    },
    undefined,
    context.subscriptions,
  );
  panel.onDidDispose(() => {
    host.dispose();
    stopFollowing();
    composer.dispose();
    chat.dispose();
    list.dispose();
  });
  return panel;
}

/** One `Published` entry, checked against the name it publishes under. */
function entry<K extends keyof ChatProjection>(
  name: K,
  read: () => ChatProjection[K],
  changes: () => Changes,
): Published {
  return { name, read, changes };
}

/**
 * Every projection this surface publishes, over the handles it holds now.
 *
 * The draft and the query are entries of their own: the webview holds its own
 * value for each until the host carries exactly that value back, which only
 * works when what crosses is the value the person typed and nothing else.
 * Identifiers cross as identifiers -- the bridge encodes `bigint` itself.
 */
function published(chat: ChatHandle, composer: ComposerHandle, list: ConversationsHandle): Published[] {
  return [
    entry(
      "chat",
      () => {
        const { error, ...rest } = chat.state();
        return { ...rest, errorKey: chat.errorKey(), errorArgs: chat.errorArgs() };
      },
      () => chat.chatChanges(),
    ),
    entry("messages", () => chat.messagesAfter(0n), () => chat.chatChanges()),
    entry(
      "composer",
      () => {
        const { error, text, ...rest } = composer.state();
        return { ...rest, errorKey: composer.errorKey(), errorArgs: composer.errorArgs() };
      },
      () => composer.composerChanges(),
    ),
    entry("draft", () => composer.state().text, () => composer.composerChanges()),
    entry("conversations", () => list.state(), () => list.listChanges()),
    entry(
      "selection",
      () => ({ selectedId: list.selectedId(), title: list.title() }),
      () => list.listChanges(),
    ),
    entry("query", () => list.query(), () => list.listChanges()),
  ];
}

function markup(webview: vscode.Webview, context: vscode.ExtensionContext, initial: Wire): string {
  const asset = (file: string) => webview.asWebviewUri(vscode.Uri.joinPath(context.extensionUri, "dist", file));
  const scriptNonce = randomBytes(16).toString("base64");
  const state = JSON.stringify({ locale: vscode.env.language, projection: initial }).replace(/</g, "\\u003c");
  return `<!doctype html>
<html lang="${vscode.env.language}">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${scriptNonce}';">
  <link rel="stylesheet" href="${asset("webview.css").toString()}">
</head>
<body>
  <main id="app"></main>
  <script nonce="${scriptNonce}">globalThis.__arutChat = ${state};</script>
  <script nonce="${scriptNonce}" src="${asset("webview.js").toString()}"></script>
</body>
</html>`;
}
