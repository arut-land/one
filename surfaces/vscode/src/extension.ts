import { createSession } from "@arut/runtime-browser";
import { registerChat } from "./surface";
import type { ExtensionContext } from "vscode";
export async function activate(context: ExtensionContext): Promise<void> {
  const session = await createSession("vscode-demo");
  context.subscriptions.push(session);
  registerChat(context, session);
}
