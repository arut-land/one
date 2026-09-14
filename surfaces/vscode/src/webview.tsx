import { uuidV7 } from "@arut/runtime-browser";
import { createSession, selectLocale, strings } from "@arut/bindings-typescript";
import { mountChat, wasmPort } from "@arut/chat-ui";

void (async () => {
  selectLocale([document.documentElement.lang]);
  const session = await createSession("vscode-demo", {
    newId: uuidV7,
    now: () => BigInt(Date.now()),
  });
  mountChat(document.querySelector<HTMLElement>("#app")!, wasmPort(session), strings);
  window.addEventListener("pagehide", () => session.dispose(), { once: true });
})();
