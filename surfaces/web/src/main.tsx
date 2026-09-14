import { uuidV7 } from "@arut/runtime-browser";
import { createSession, strings } from "@arut/bindings-typescript";
import { mountChat, wasmPort } from "@arut/chat-ui";

// Strings are negotiated from navigator.languages as the module loads (ADR
// 0022), so the only thing this root waits for is the wasm core.
const session = await createSession("local-demo", { newId: uuidV7, now: () => BigInt(Date.now()) });
mountChat(document.querySelector<HTMLElement>("#app")!, wasmPort(session), strings);
window.addEventListener("pagehide", () => session.dispose(), { once: true });
