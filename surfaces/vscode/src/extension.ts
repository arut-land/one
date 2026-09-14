import { uuidV7 } from "@arut/runtime-browser";
import { createSession, selectLocale } from "@arut/bindings-typescript";
import { env, type ExtensionContext } from "vscode";
import { registerChat } from "./host";

export async function activate(context: ExtensionContext): Promise<void> {
  // The editor owns the language; the catalog is compiled in (ADR 0022), so
  // negotiating it is a call rather than a read of files beside the extension.
  selectLocale([env.language]);
  const session = await createSession("vscode-demo", { newId: uuidV7, now: () => BigInt(Date.now()) });
  context.subscriptions.push(session);
  registerChat(context, session);
}
