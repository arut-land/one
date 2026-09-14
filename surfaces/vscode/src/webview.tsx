import type { HostMessage, PortMessage, Wire } from "@arut/bindings-typescript/bridge";
import { selectLocale, strings } from "@arut/bindings-typescript/strings";
import { bridgePort, mountChat } from "@arut/chat-ui";

declare function acquireVsCodeApi(): { postMessage(message: PortMessage): void };

const opened = (globalThis as { __arutChat?: { locale: string; projection: Wire } }).__arutChat;
if (opened === undefined) throw new Error("the extension host renders the first projection into the page");

selectLocale([opened.locale]);
const editor = acquireVsCodeApi();
const port = bridgePort(message => editor.postMessage(message), opened.projection);
mountChat(document.querySelector<HTMLElement>("#app")!, port, strings);
window.addEventListener("message", (event: MessageEvent<HostMessage>) => {
  if (event.data.type === "projection") port.receive(event.data.values);
});
