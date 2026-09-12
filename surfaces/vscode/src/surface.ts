import { randomBytes } from "node:crypto";
import { followComposer, type ChatHandle, describeChatError, describeComposerError, type ProductSessionHandle } from "@arut/bindings-typescript";
import * as vscode from "vscode";

function nonce(): string {
  return randomBytes(16).toString("base64");
}

export function registerChat(context: vscode.ExtensionContext, session: ProductSessionHandle): void {
  let panel: vscode.WebviewPanel | undefined;
  context.subscriptions.push(vscode.commands.registerCommand("arut.chat", () => {
    if (panel) { panel.reveal(vscode.ViewColumn.Beside); return; }
    panel = vscode.window.createWebviewPanel("arut.chat", "Arut", vscode.ViewColumn.Beside, {
      enableScripts: true,
      // The core keeps running host-side; retaining the webview's DOM avoids
      // re-running composer.initialize()/follow() every time the panel is hidden.
      retainContextWhenHidden: true,
    });
    panel.webview.html = markup();
    const current = panel;
    let chat = session.chat();
    let composer = chat.composer();
    let lastPostedId = 0n;
    let generation = 0;
    const list = session.conversations();
    // Errors carry bigint payloads (a revision, an epoch) that the webview's
    // JSON postMessage channel cannot serialize, so the extension host
    // resolves each typed error to its sentence before it crosses the wire.
    const publish = () => {
      const chatState = chat.state();
      const added = chat.messagesAfter(lastPostedId);
      lastPostedId = added.at(-1)?.id ?? lastPostedId;
      const composerState = composer.state();
      const error = chatState.error
        ? describeChatError(chatState.error)
        : composerState.error
          ? describeComposerError(composerState.error)
          : null;
      current.webview.postMessage({ type: "state", state: {
        generation, status: chatState.status, chatId: chatState.id, draft: composerState.text, history: list.state(), error,
        messages: added.map(message => ({ ...message, id: message.id.toString() })),
      } });
    };
    let chatChanges = chat.chatChanges(() => { void publish(); });
    let composerChanges = composer.composerChanges(() => { void publish(); });
    const listChanges = list.listChanges(() => { void publish(); });
    let stopFollowing = followComposer(composer);
    const bind = (next: ChatHandle) => {
      chatChanges.cancel(); composerChanges.cancel(); stopFollowing();
      composer.dispose(); chat.dispose();
      chat = next;
      composer = chat.composer();
      lastPostedId = 0n;
      generation++;
      chatChanges = chat.chatChanges(() => { void publish(); });
      composerChanges = composer.composerChanges(() => { void publish(); });
      stopFollowing = followComposer(composer);
      void publish();
    };
    current.webview.onDidReceiveMessage(async (message: { type: string; text: string; chatId: string }) => {
      if (message.type === "ready") { lastPostedId = 0n; generation++; publish(); }
      else if (message.type === "newChat") { bind(session.newChat()); }
      else if (message.type === "selectChat") { const next = session.selectChat(message.chatId); if (next) { bind(next); } }
      else if (message.type === "draft") { await composer.replace(message.text); }
      else if (message.type === "send") { await chat.send(message.text); }
    }, undefined, context.subscriptions);
    current.onDidDispose(() => { chatChanges.cancel(); composerChanges.cancel(); listChanges.cancel(); stopFollowing(); composer.dispose(); chat.dispose(); list.dispose(); panel = undefined; });
    void publish();
  }));
}

function markup(): string {
  const scriptNonce = nonce();
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'nonce-${scriptNonce}';">
  <style>
    * { box-sizing: border-box; }
    body { --sidebar: 210px; display: grid; grid-template-columns: var(--sidebar) minmax(0, 1fr); grid-template-rows: 58px minmax(0, 1fr) auto; height: 100vh; margin: 0; color: var(--vscode-foreground); background: var(--vscode-editor-background); font-family: var(--vscode-font-family); transition: grid-template-columns 160ms ease; }
    body.sidebar-closed { --sidebar: 0px; }
    header { z-index: 3; display: flex; grid-column: 1 / -1; align-items: center; gap: 10px; padding: 9px 14px; border-bottom: 1px solid var(--vscode-panel-border); background: var(--vscode-editor-background); }
    header strong { overflow: hidden; flex: 1; font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
    button, input { color: inherit; font: inherit; }
    header button { min-width: 32px; height: 32px; border: 0; border-radius: 5px; color: var(--vscode-foreground); background: none; cursor: pointer; }
    header button:hover { background: var(--vscode-toolbar-hoverBackground); }
    header button svg { width: 16px; height: 16px; vertical-align: middle; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
    #new-chat { width: auto; padding-inline: 10px; color: var(--vscode-textLink-foreground); }
    #history { z-index: 2; display: flex; grid-row: 2 / 4; min-width: 0; flex-direction: column; gap: 3px; overflow: hidden auto; padding: 14px 8px; border-right: 1px solid var(--vscode-panel-border); background: var(--vscode-sideBar-background); transition: opacity 100ms ease; }
    .sidebar-closed #history { visibility: hidden; opacity: 0; }
    #history > span { padding: 0 8px 8px; color: var(--vscode-descriptionForeground); font-size: 10px; letter-spacing: .12em; text-transform: uppercase; }
    #history button { overflow: hidden; min-height: 34px; border: 0; border-radius: 5px; padding: 8px 10px; background: transparent; text-align: left; text-overflow: ellipsis; white-space: nowrap; cursor: pointer; }
    #history button:hover { background: var(--vscode-list-hoverBackground); }
    #history button.active { background: var(--vscode-list-activeSelectionBackground); color: var(--vscode-list-activeSelectionForeground); }
    #messages { display: flex; min-width: 0; flex-direction: column; gap: 20px; overflow-y: auto; padding: clamp(18px, 4vw, 36px); }
    .empty { display: grid; margin: auto; justify-items: center; gap: 8px; color: var(--vscode-descriptionForeground); }
    .empty::before { display: grid; width: 42px; height: 42px; place-items: center; border-radius: 10px; background: var(--vscode-button-background); color: var(--vscode-button-foreground); font-size: 17px; content: 'A'; }
    article { max-width: 82%; }
    article.user { align-self: flex-end; padding: 10px 13px; border-radius: 13px 4px 13px 13px; background: var(--vscode-button-background); color: var(--vscode-button-foreground); }
    article span { display: block; margin-bottom: 4px; color: var(--vscode-descriptionForeground); font-size: 11px; }
    article.user span { color: inherit; opacity: .75; }
    article p { margin: 0; line-height: 1.5; white-space: pre-wrap; }
    #error { grid-column: 2; margin: 0 14px; color: var(--vscode-errorForeground); font-size: 12px; }
    form { display: flex; grid-column: 2; gap: 8px; margin: 10px 14px 14px; padding: 5px 5px 5px 12px; border: 1px solid var(--vscode-input-border); border-radius: 9px; background: var(--vscode-input-background); }
    input { min-width: 0; flex: 1; border: 0; background: transparent; color: var(--vscode-input-foreground); outline: none; }
    form button { border: 0; border-radius: 6px; padding: 0 14px; background: var(--vscode-button-background); color: var(--vscode-button-foreground); cursor: pointer; }
    button:disabled { opacity: .5; cursor: default; }
    #scrim { display: none; }
    @media (max-width: 560px) {
      body, body.sidebar-closed { --sidebar: 0px; }
      #history { position: fixed; inset: 58px auto 0 0; width: min(82vw, 260px); box-shadow: 12px 0 32px #0005; }
      .sidebar-closed #history { transform: translateX(-105%); }
      body:not(.sidebar-closed) #scrim { position: fixed; z-index: 1; inset: 58px 0 0; display: block; border: 0; background: #0005; }
      form { grid-column: 1; }
    }
    @media (prefers-reduced-motion: reduce) { * { transition-duration: .01ms !important; } }
  </style>
</head>
<body>
  <header><button id="toggle-history" type="button" title="Toggle chat history (Alt+S)" aria-label="Toggle chat history"><svg aria-hidden="true" viewBox="0 0 24 24"><path d="M4 5.5h16v13H4zM9 5.5v13" /></svg></button><strong id="title">New conversation</strong><button id="new-chat" type="button" title="New chat (Alt+N)"><svg aria-hidden="true" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg> New chat</button></header>
  <button id="scrim" type="button" aria-label="Close chat history"></button>
  <aside id="history"><span>Recent</span></aside>
  <main id="messages"><p class="empty">Start a conversation.</p></main>
  <p id="error" hidden></p>
  <form><input autofocus aria-label="Message Arut" placeholder="Message Arut"><button>Send</button></form>
  <script nonce="${scriptNonce}">
    const vscode = acquireVsCodeApi();
    const messages = document.querySelector('#messages');
    const history = document.querySelector('#history');
    const form = document.querySelector('form');
    const input = document.querySelector('input');
    const send = form.querySelector('button');
    const toggle = document.querySelector('#toggle-history');
    const scrim = document.querySelector('#scrim');
    const title = document.querySelector('#title');
    const error = document.querySelector('#error');
    const saved = vscode.getState() || {};
    let sidebarOpen = saved.sidebarOpen ?? window.innerWidth > 560;
    const applySidebar = () => {
      document.body.classList.toggle('sidebar-closed', !sidebarOpen);
      toggle.setAttribute('aria-expanded', String(sidebarOpen));
      vscode.setState({ ...saved, sidebarOpen });
    };
    const setSidebar = open => { sidebarOpen = open; applySidebar(); };
    applySidebar();
    toggle.addEventListener('click', () => setSidebar(!sidebarOpen));
    scrim.addEventListener('click', () => setSidebar(false));
    form.addEventListener('submit', event => {
      event.preventDefault();
      const text = input.value.trim();
      if (!text || send.disabled) return;
      input.value = '';
      send.disabled = true;
      vscode.postMessage({ type: 'send', text });
    });
    input.addEventListener('input', () => vscode.postMessage({ type: 'draft', text: input.value }));
    document.querySelector('#new-chat').addEventListener('click', () => {
      vscode.postMessage({ type: 'newChat' });
      input.focus();
    });
    window.addEventListener('keydown', event => {
      if (event.key === 'Escape' && sidebarOpen) setSidebar(false);
      if (event.altKey && event.key.toLowerCase() === 's') { event.preventDefault(); setSidebar(!sidebarOpen); }
      if (event.altKey && event.key.toLowerCase() === 'n') { event.preventDefault(); document.querySelector('#new-chat').click(); }
      if (event.key === '/' && document.activeElement !== input) { event.preventDefault(); input.focus(); }
    });
    let generation = -1;
    window.addEventListener('message', ({ data }) => {
      if (data.type !== 'state') return;
      if (generation !== data.state.generation) {
        generation = data.state.generation;
        messages.replaceChildren();
      }
      history.replaceChildren();
      const historyLabel = document.createElement('span');
      historyLabel.textContent = 'Recent';
      history.append(historyLabel);
      for (const chat of data.state.history) {
        const item = document.createElement('button');
        item.type = 'button';
        item.textContent = chat.title;
        item.title = chat.title;
        item.className = data.state.chatId === chat.id ? 'active' : '';
        item.addEventListener('click', () => {
          vscode.postMessage({ type: 'selectChat', chatId: chat.id });
          if (window.innerWidth <= 560) setSidebar(false);
        });
        history.append(item);
      }
      title.textContent = data.state.history.find(chat => chat.id === data.state.chatId)?.title || 'New conversation';
      if (data.state.messages.length > 0) messages.querySelector('.empty')?.remove();
      if (!messages.firstChild && data.state.messages.length === 0) {
        const empty = document.createElement('p');
        empty.className = 'empty';
        empty.textContent = 'Start a conversation.';
        messages.append(empty);
      }
      for (const message of data.state.messages) {
        const item = document.createElement('article');
        item.className = message.role === 0 ? 'user' : 'assistant';
        const role = document.createElement('span');
        role.textContent = message.role === 0 ? 'You' : 'Arut';
        const text = document.createElement('p');
        text.textContent = message.text;
        item.append(role, text);
        messages.append(item);
      }
      send.disabled = data.state.status === 1;
      error.hidden = !data.state.error;
      error.textContent = data.state.error || '';
      if (input.value !== data.state.draft) input.value = data.state.draft;
      messages.scrollTop = messages.scrollHeight;
    });
    vscode.postMessage({ type: 'ready' });
  </script>
</body>
</html>`;
}
