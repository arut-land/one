import { browserApp } from "@arut/chat-ui/vite";
import { defineConfig } from "vite";

// Two bundles from one root, and one `vite build` makes both: `client` owns the
// Wasm session in the webview, while `ssr` only registers and opens the panel.
export default defineConfig(
  browserApp({
    builder: {},
    environments: {
      // A library build leaves `process.env` to its consumer; a webview has no
      // `process`, so React's build switch is resolved here.
      client: {
        define: { "process.env.NODE_ENV": JSON.stringify("production") },
        build: {
          lib: { entry: "src/webview.tsx", formats: ["es"], cssFileName: "webview", fileName: () => "webview.js" },
        },
      },
      ssr: {
        consumer: "client",
        build: {
          emptyOutDir: false,
          lib: { entry: "src/extension.ts", formats: ["es"], fileName: "extension" },
          rolldownOptions: { external: ["vscode", "node:crypto"] },
        },
      },
    },
  }),
);
