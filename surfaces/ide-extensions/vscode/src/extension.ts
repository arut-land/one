import { createChatController } from "@arut/bindings-typescript";
import { registerChat } from "./surface";
import type { ExtensionContext } from "vscode";

export async function activate(context: ExtensionContext): Promise<void> {
  const chat = await createChatController();
  context.subscriptions.push(chat);
  registerChat(context, chat);
}
