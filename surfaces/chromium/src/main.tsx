import { uuidV7 } from "@arut/runtime-browser";
import { createSession, strings } from "@arut/bindings-typescript";
import { mountChat, wasmPort } from "@arut/chat-ui";

// The popup is its own root, not the web page's: an extension owns its
// lifecycle and its manifest, and Firefox and Safari are siblings of this file.
const session = await createSession("chromium-popup", { newId: uuidV7, now: () => BigInt(Date.now()) });
mountChat(document.querySelector<HTMLElement>("#app")!, wasmPort(session), strings);
window.addEventListener("pagehide", () => session.dispose(), { once: true });
