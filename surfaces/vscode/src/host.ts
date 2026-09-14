import { randomBytes } from "node:crypto";
import {
  followComposer,
  observe,
  strings,
  t,
  type ChatHandle,
  type ObservableStore,
  type ProductSessionHandle,
} from "@arut/bindings-typescript";
import type { ChatCommand, ChatProjection } from "@arut/chat-ui/port";
import * as vscode from "vscode";

// The session stays in the extension host (ADR 0011); the webview is a view
// over it. What crosses is one whole projection per change, and typed commands
// back -- no sentence and no handle (ADR 0016).

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

  const list = session.conversations();
  const conversations = observe(
    () => ({ conversations: list.state(), selectedId: list.selectedId() }),
    () => list.listChanges(),
  );
  let chat = session.chat();
  let composer = chat.composer();
  let stopFollowing = followComposer(composer);
  let transcript = observe(readChat(chat), () => chat.chatChanges());
  let draft = observe(readComposer(composer), () => composer.composerChanges());
  const project = (): ChatProjection => ({
    ...transcript.getSnapshot(),
    composer: draft.getSnapshot(),
    ...conversations.getSnapshot(),
  });
  const publish = () => void panel.webview.postMessage(project());
  const stop = [conversations.subscribe(publish), transcript.subscribe(publish), draft.subscribe(publish)];

  const bind = (next: ChatHandle) => {
    release([transcript, draft]);
    stopFollowing();
    composer.dispose();
    chat.dispose();
    chat = next;
    composer = chat.composer();
    stopFollowing = followComposer(composer);
    transcript = observe(readChat(chat), () => chat.chatChanges());
    draft = observe(readComposer(composer), () => composer.composerChanges());
    stop[1] = transcript.subscribe(publish);
    stop[2] = draft.subscribe(publish);
    publish();
  };

  panel.webview.html = markup(panel.webview, context, project());
  panel.webview.onDidReceiveMessage(
    (command: ChatCommand) => {
      if (command.type === "ready") publish();
      else if (command.type === "newChat") bind(session.newChat());
      else if (command.type === "select") {
        list.select(command.id);
        const next = command.id === null ? undefined : session.selectChat(command.id);
        if (next) bind(next);
      } else if (command.type === "draft") void composer.replace(command.text);
      else if (command.type === "send") void chat.send(command.text);
    },
    undefined,
    context.subscriptions,
  );
  panel.onDidDispose(() => {
    for (const unsubscribe of stop) unsubscribe();
    stopFollowing();
    release([conversations, transcript, draft]);
    composer.dispose();
    chat.dispose();
    list.dispose();
  });
  return panel;
}

function release(stores: ObservableStore<unknown>[]): void {
  for (const store of stores) store.dispose();
}

/** The chat half of a projection: identifiers as text, errors as Fluent ids. */
function readChat(chat: ChatHandle): () => Pick<ChatProjection, "chat" | "messages"> {
  return () => {
    const { error, lastMessageId, ...rest } = chat.state();
    return {
      chat: { ...rest, lastMessageId: lastMessageId.toString(), ...errorOf(chat) },
      messages: chat.messagesAfter(0n).map(message => ({
        ...message,
        id: message.id.toString(),
        acceptedAtMs: message.acceptedAtMs.toString(),
      })),
    };
  };
}

function readComposer(composer: ReturnType<ChatHandle["composer"]>): () => ChatProjection["composer"] {
  return () => {
    const { error, revision, ...rest } = composer.state();
    return { ...rest, revision: revision.toString(), ...errorOf(composer) };
  };
}

/** The typed error as the pair a surface renders from (ADR 0016). */
function errorOf(handle: { errorKey(): string | null; errorArgs(): string[] }) {
  return { errorKey: handle.errorKey(), errorArgs: handle.errorArgs() };
}

function markup(webview: vscode.Webview, context: vscode.ExtensionContext, initial: ChatProjection): string {
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
