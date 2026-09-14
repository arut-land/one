import type { ExtensionContext } from "vscode";
import { registerChat } from "./host";

export function activate(context: ExtensionContext): void {
  registerChat(context);
}
