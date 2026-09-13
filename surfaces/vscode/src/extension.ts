import { uuidV7 } from "../../../runtimes/browser/ids";
import { createSession, loadStrings } from "@arut/bindings-typescript";
import { registerChat } from "./surface";
import { Uri, env, workspace, type ExtensionContext } from "vscode";
export async function activate(context: ExtensionContext): Promise<void> {
  // The extension host resolves typed errors to sentences before they cross to
  // the webview, so it loads the shared Fluent source itself, through its own
  // resource URIs rather than over HTTP (ADR 0022).
  await loadStrings({
    preferred: [env.language],
    read: async file => new TextDecoder().decode(
      await workspace.fs.readFile(Uri.joinPath(context.extensionUri, "locales", ...file.split("/"))),
    ),
  });
  const session = await createSession("vscode-demo", { newId: uuidV7 });
  context.subscriptions.push(session);
  registerChat(context, session);
}
