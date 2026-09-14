import { selectLocale, strings } from "@arut/bindings-typescript/strings";
import { bridgePort, mountChat, type ChatCommand, type ChatProjection } from "@arut/chat-ui";

declare function acquireVsCodeApi(): { postMessage(message: ChatCommand): void };

const opened = (globalThis as { __arutChat?: { locale: string; projection: ChatProjection } }).__arutChat;
if (opened === undefined) throw new Error("the extension host renders the first projection into the page");

selectLocale([opened.locale]);
const editor = acquireVsCodeApi();
const port = bridgePort(command => editor.postMessage(command), opened.projection);
mountChat(document.querySelector<HTMLElement>("#app")!, port, strings);
window.addEventListener("message", (event: MessageEvent<ChatProjection>) => port.receive(event.data));
editor.postMessage({ type: "ready" });
