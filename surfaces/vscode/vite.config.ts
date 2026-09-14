import { browserApp } from "@arut/chat-ui/vite";
import { defineConfig } from "vite";

// Two bundles from one root, and one `vite build` makes both: Vite's own pair of
// environments is this surface's pair of sides -- `client` is the webview page,
// `ssr` the extension host module that owns the session (ADR 0011).
export default defineConfig(
  browserApp({
    builder: {},
    environments: {
      // A library build leaves `process.env` to its consumer; a webview has no
      // `process`, so React's build switch is resolved here.
      client: {
        define: { "process.env.NODE_ENV": JSON.stringify("production") },
        build: {
          lib: { entry: "src/webview.tsx", formats: ["iife"], name: "arutChat", cssFileName: "webview", fileName: () => "webview.js" },
        },
      },
      // The host resolves like a browser because the wasm it loads is the
      // browser build, inlined so the extension ships as the one file.
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
